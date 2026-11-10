// generated from _validate.mts by scripts/ts_emit.ts: never hand-edited (edit the .ts, then run it)
// The allow list of bise.dev's telemetry endpoint: a body is taken only when every field, key and
// value is one the app's crate sends (rust/usage, its schema exported to rust/usage/schema.json and
// copied byte for byte into _schema.mts). This reads the schema's types; it never lists a key itself,
// so a key the crate adds is taken once the copy follows (tests/site_telemetry.test.ts pins both).
// The same checks as the crate's PII law (rust/usage/src/law.rs), plus the day's window. Pure: no
// request, no clock (today is passed in), no store.
import { SCHEMA } from "./_schema.mjs";
const spec = SCHEMA;
/** The oldest day taken, in days before today: the app's spool keeps 30 (docs/telemetry.md). */
export const OLDEST_DAYS = 40;
const fail = (error) => ({ ok: false, error });
const isObject = (v) => typeof v === "object" && v !== null && !Array.isArray(v);
const isWhole = (v, min, max) => typeof v === "number" && Number.isInteger(v) && v >= min && v <= max;
/** A calendar day `YYYY-MM-DD` (the crate's Day::parse). */
export function isDay(s) {
    if (typeof s !== "string" || !/^\d{4}-\d{2}-\d{2}$/.test(s))
        return false;
    const [y, m, d] = s.split("-").map(Number);
    const t = new Date(Date.UTC(y, m - 1, d));
    return t.getUTCFullYear() === y && t.getUTCMonth() === m - 1 && t.getUTCDate() === d;
}
/** `day` moved by `n` days (UTC). */
export function addDays(day, n) {
    const [y, m, d] = day.split("-").map(Number);
    return new Date(Date.UTC(y, m - 1, d + n)).toISOString().slice(0, 10);
}
/** `day` moved back by `n` months, the day of the month clamped (2026-03-31 - 1 = 2026-02-28). */
export function monthsBefore(day, n) {
    const [y, m, d] = day.split("-").map(Number);
    const first = new Date(Date.UTC(y, m - 1 - n, 1));
    const last = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth() + 1, 0)).getUTCDate();
    return new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth(), Math.min(d, last))).toISOString().slice(0, 10);
}
/** The UTC day of an instant. */
export const dayOf = (ms) => new Date(ms).toISOString().slice(0, 10);
const isIsoWeek = (s) => {
    const m = typeof s === "string" ? /^(\d{4})-W(\d{2})$/.exec(s) : null;
    return m !== null && Number(m[2]) >= 1 && Number(m[2]) <= 53;
};
/** A random UUID v4 in lower case (the crate's is_uuid_v4). */
export const isUuid4 = (s) => typeof s === "string" && /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(s);
const isVersion = (s) => typeof s === "string" && /^[0-9a-z.-]{1,24}$/.test(s);
/** No string and no key anywhere holds a `/` or an `@` (the law's own rule, under the typed checks). */
function noSlashOrAt(v) {
    if (typeof v === "string")
        return !v.includes("/") && !v.includes("@");
    if (Array.isArray(v))
        return v.every(noSlashOrAt);
    if (isObject(v))
        return Object.entries(v).every(([k, x]) => !k.includes("/") && !k.includes("@") && noSlashOrAt(x));
    return true;
}
function checkField(name, v, f) {
    const bad = `${name} is not ${f.type === "enum" ? "one of its values" : `a valid ${f.type}`}`;
    switch (f.type) {
        case "integer":
            return isWhole(v, f.min ?? 0, f.max ?? 0) ? null : `${name} is out of its bounds`;
        case "boolean":
            return typeof v === "boolean" ? null : bad;
        case "day":
            return isDay(v) ? null : bad;
        case "iso_week":
            return isIsoWeek(v) ? null : bad;
        case "uuid4":
            return isUuid4(v) ? null : bad;
        case "version":
            return isVersion(v) ? null : bad;
        case "enum":
            return typeof v === "string" && (f.values ?? []).includes(v) ? null : bad;
        case "counts":
        case "minutes": {
            if (!isObject(v))
                return `${name} is not an object`;
            const keys = f.keys;
            for (const [k, x] of Object.entries(v)) {
                const ks = Object.hasOwn(keys, k) ? keys[k] : undefined;
                if (!ks)
                    return `${name} has a key not in the schema`;
                if (!isWhole(x, 1, ks.max))
                    return `${name}.${k} is out of its bounds`;
                if (x % (ks.step ?? 1) !== 0)
                    return `${name}.${k} is not a multiple of ${ks.step}`;
            }
            return null;
        }
        case "hist": {
            if (!isObject(v))
                return `${name} is not an object`;
            const keys = f.keys;
            for (const [k, x] of Object.entries(v)) {
                const ks = Object.hasOwn(keys, k) ? keys[k] : undefined;
                if (!ks)
                    return `${name} has a key not in the schema`;
                if (!Array.isArray(x) || x.length !== (ks.buckets ?? []).length)
                    return `${name}.${k} has the wrong buckets`;
                if (!x.every((b) => isWhole(b, 0, ks.max)))
                    return `${name}.${k} is out of its bounds`;
            }
            return null;
        }
        case "milestones": {
            if (!isObject(v))
                return `${name} is not an object`;
            const keys = f.keys;
            for (const [k, d] of Object.entries(v)) {
                if (!keys.includes(k))
                    return `${name} has a key not in the schema`;
                if (typeof d !== "string" || !(f.values ?? []).includes(d))
                    return `${name}.${k} is not one of its values`;
            }
            return null;
        }
        default:
            return `the schema's type ${f.type} has no check`;
    }
}
function checkPacket(p, today) {
    if (!isObject(p))
        return "a packet is not an object";
    for (const k of Object.keys(p))
        if (!Object.hasOwn(spec.packet, k))
            return "a packet has a field not in the schema";
    for (const [name, f] of Object.entries(spec.packet)) {
        if (!Object.hasOwn(p, name))
            return `a packet has no ${name}`;
        const error = checkField(name, p[name], f);
        if (error)
            return error;
    }
    const day = p.day;
    if (day >= today)
        return "a packet's day is not closed yet (UTC)";
    if (day < addDays(today, -OLDEST_DAYS))
        return `a packet's day is more than ${OLDEST_DAYS} days old`;
    return null;
}
/** A `POST /v1/packets` body, parsed: its packets when every one is in the schema, else why not. */
export function checkPackets(body, today) {
    if (!isObject(body))
        return fail("the body is not an object");
    if (Object.keys(body).some((k) => k !== "packets"))
        return fail("the body has a field not in the schema");
    const packets = body.packets;
    if (!Array.isArray(packets) || packets.length === 0)
        return fail("the body has no packets");
    if (packets.length > spec.body.packets.max_items)
        return fail(`more than ${spec.body.packets.max_items} packets`);
    if (!noSlashOrAt(body))
        return fail("a string holds a / or an @");
    for (const p of packets) {
        const error = checkPacket(p, today);
        if (error)
            return fail(error);
    }
    return { ok: true, value: packets };
}
/** A `POST /v1/delete` body: its install id, nothing else. */
export function checkDelete(body) {
    if (!isObject(body) || Object.keys(body).some((k) => k !== "install_id"))
        return fail("the body is not {install_id}");
    return isUuid4(body.install_id) ? { ok: true, value: body.install_id } : fail("install_id is not a uuid v4");
}
