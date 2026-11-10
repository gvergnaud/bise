// The telemetry store: one Postgres table of the packets the app sends (one row per install and UTC
// day), and what reads and deletes it. The table is made at first use (CREATE ... IF NOT EXISTS).
// Rows older than 13 months are deleted at each ingest and by the daily cron (one fn, `purge`), so the
// promise holds on a day no packet comes. The dashboard's reads count installs per cell and never
// select install_id: the id leaves the table only to be deleted.
import type { Packet } from "./_validate.mjs";
import { addDays, monthsBefore } from "./_validate.mjs";
import type { Row, Sql } from "./_sql.mjs";

/** How long a packet is kept, in months (the CNIL audience-measurement exception's 13). */
export const KEEP_MONTHS = 13;

/** The first day kept on `today`: older rows are deleted. */
export const keptFrom = (today: string): string => monthsBefore(today, KEEP_MONTHS);

const COLUMNS = `install_id uuid, day date, schema smallint, cohort_week text, active_days_7 smallint,
  active_days_28 smallint, first_today_of_week boolean, version text, channel text, surface text,
  os_major smallint, arch text, counts jsonb, minutes jsonb, hist jsonb, milestones jsonb`;
const NAMES = COLUMNS.split(",").map((c) => c.trim().split(/\s+/)[0]).join(", ");

export const MIGRATION = [
  `create table if not exists packets (
    install_id uuid not null, day date not null, schema smallint not null, cohort_week text not null,
    active_days_7 smallint not null, active_days_28 smallint not null, first_today_of_week boolean not null,
    version text not null, channel text not null, surface text not null, os_major smallint not null,
    arch text not null, counts jsonb not null, minutes jsonb not null, hist jsonb not null,
    milestones jsonb not null,
    primary key (install_id, day))`,
  `create index if not exists packets_day on packets (day)`,
];

const ready = new WeakSet<Sql>();

/** The table, made once per warm function. */
export async function ensureTable(sql: Sql): Promise<void> {
  if (ready.has(sql)) return;
  for (const q of MIGRATION) await sql(q);
  ready.add(sql);
}

const num = (v: string | null | undefined): number => (v == null ? 0 : Number(v));

/** Store the packets (a packet already there for its install and day is kept as it was: a resend
 * after a timeout counts once), and purge what is past retention. */
export async function storePackets(sql: Sql, packets: Packet[], today: string): Promise<{ stored: number; purged: number }> {
  await ensureTable(sql);
  const [r] = await sql(
    `with purged as (delete from packets where day < $2::date returning 1),
     ins as (
       insert into packets (${NAMES})
       select ${NAMES} from jsonb_to_recordset($1::jsonb) as x(${COLUMNS})
       on conflict (install_id, day) do nothing
       returning 1)
     select (select count(*) from ins) as stored, (select count(*) from purged) as purged`,
    [JSON.stringify(packets), keptFrom(today)],
  );
  return { stored: num(r?.stored), purged: num(r?.purged) };
}

/** "Delete my data": every packet of that install. */
export async function deleteInstall(sql: Sql, installId: string): Promise<number> {
  await ensureTable(sql);
  const [r] = await sql(`with d as (delete from packets where install_id = $1::uuid returning 1) select count(*) as deleted from d`, [installId]);
  return num(r?.deleted);
}

/** Delete the packets past retention on `today`. */
export async function purge(sql: Sql, today: string): Promise<number> {
  await ensureTable(sql);
  const [r] = await sql(`with d as (delete from packets where day < $1::date returning 1) select count(*) as purged from d`, [keptFrom(today)]);
  return num(r?.purged);
}

// ---- the dashboard's reads ----------------------------------------------------------------------

/** A number and how many installs it comes from (the dashboard hides it under 10). */
export interface Cell {
  value: number;
  installs: number;
}

export interface DayRow {
  day: string;
  reporting: Cell;
  active: Cell;
  agents: Cell;
  byUser: Cell;
  byMain: Cell;
  byAgent: Cell;
  agentHours: Cell;
  crashes: Cell;
}

export interface Stats {
  /** The last closed day, and the first of the 30 shown. */
  last: string;
  first: string;
  days: DayRow[];
  wau: Cell;
  mau: Cell;
  agentHours7: Cell;
  focusedHours7: Cell;
  versions: { version: string; channel: string; installs: number }[];
  surfaces: { surface: string; installs: number }[];
  quality: { version: string; installs: number; installDays: number; crashFreeDays: number; crashes: number }[];
  attention: { open: Cell; focused: Cell; engaged: Cell };
  setup: { started: Cell; done: Cell; failedAuth: Cell; failedNetwork: Cell; failedOther: Cell };
}

const int = (json: string, key: string) => `coalesce((${json}->>'${key}')::int, 0)`;

/** One row per install and day in [$1, $2), with the numbers the reads use. No text leaves it but
 * the version, channel and surface, from closed lists. */
const P = `p as (
  select install_id, day, version, channel, surface,
    ${int("counts", "user_message")} + ${int("counts", "card_answered")} > 0 as active,
    ${int("counts", "agent_created_by_user")} as by_user,
    ${int("counts", "agent_created_by_main")} as by_main,
    ${int("counts", "agent_created_by_agent")} as by_agent,
    ${int("minutes", "agent_work")} as agent_work,
    ${int("minutes", "app_open")} as app_open,
    ${int("minutes", "app_focused")} as app_focused,
    ${int("minutes", "app_engaged")} as app_engaged,
    ${int("minutes", "tui_focused")} as tui_focused,
    ${int("counts", "crash_tui")} + ${int("counts", "crash_app_renderer")} + ${int("counts", "crash_app_child")}
      + ${int("counts", "crash_core")} + ${int("counts", "crash_hub")} as crashes,
    ${int("counts", "setup_started")} as setup_started,
    ${int("counts", "setup_done")} as setup_done,
    ${int("counts", "setup_failed_auth")} as setup_failed_auth,
    ${int("counts", "setup_failed_network")} as setup_failed_network,
    ${int("counts", "setup_failed_other")} as setup_failed_other
  from packets where day >= $1::date and day < $2::date)`;

/** A column summed per day, and how many installs gave it something (one row per install a day). */
const summed = (expr: string, name: string) => `coalesce(sum(${expr}), 0) as ${name}, count(*) filter (where ${expr} > 0) as ${name}_n`;
const installsWhere = (cond: string, name: string) => `count(distinct install_id) filter (where ${cond}) as ${name}`;

const cell = (r: Row | undefined, name: string, basis = `${name}_n`): Cell => ({ value: num(r?.[name]), installs: num(r?.[basis]) });
const own = (r: Row | undefined, name: string): Cell => ({ value: num(r?.[name]), installs: num(r?.[name]) });

/** The numbers of the 30 closed days before `today`. */
export async function readStats(sql: Sql, today: string): Promise<Stats> {
  await ensureTable(sql);
  const last = addDays(today, -1);
  const first = addDays(today, -30);
  const week = addDays(today, -7);
  const month = addDays(today, -28);
  const [days, [win], versions, surfaces, quality, [attention], [setup]] = await Promise.all([
    sql(
      `with ${P} select day::text as day, count(*) as reporting, count(*) filter (where active) as active,
        ${summed("by_user + by_main + by_agent", "agents")}, ${summed("by_user", "by_user")},
        ${summed("by_main", "by_main")}, ${summed("by_agent", "by_agent")},
        ${summed("agent_work", "agent_work")}, ${summed("crashes", "crashes")}
       from p group by day order by day`,
      [first, today],
    ),
    sql(
      `with ${P} select ${installsWhere("active and day >= $3::date", "wau")}, ${installsWhere("active", "mau")},
        coalesce(sum(agent_work) filter (where day >= $3::date), 0) as agent_work,
        ${installsWhere("agent_work > 0 and day >= $3::date", "agent_work_n")},
        coalesce(sum(app_focused + tui_focused) filter (where day >= $3::date), 0) as focused,
        ${installsWhere("app_focused + tui_focused > 0 and day >= $3::date", "focused_n")}
       from p`,
      [month, today, week],
    ),
    sql(`with ${P} select version, channel, count(distinct install_id) as installs from p group by version, channel order by installs desc, version desc`, [week, today]),
    sql(`with ${P} select surface, count(distinct install_id) as installs from p group by surface order by installs desc, surface`, [week, today]),
    sql(
      `with ${P} select version, count(distinct install_id) as installs, count(*) as install_days,
        count(*) filter (where crashes = 0) as crash_free_days, coalesce(sum(crashes), 0) as crashes
       from p group by version order by installs desc, version desc`,
      [week, today],
    ),
    sql(
      `with ${P} select
        round(coalesce(percentile_cont(0.5) within group (order by app_open), 0)) as open,
        round(coalesce(percentile_cont(0.5) within group (order by app_focused), 0)) as focused,
        round(coalesce(percentile_cont(0.5) within group (order by app_engaged), 0)) as engaged,
        count(distinct install_id) as n
       from p where app_open > 0`,
      [week, today],
    ),
    sql(
      `with ${P} select ${installsWhere("setup_started > 0", "started")}, ${installsWhere("setup_done > 0", "done")},
        ${installsWhere("setup_failed_auth > 0", "failed_auth")}, ${installsWhere("setup_failed_network > 0", "failed_network")},
        ${installsWhere("setup_failed_other > 0", "failed_other")}
       from p`,
      [month, today],
    ),
  ]);
  const hours = (c: Cell): Cell => ({ value: Math.round(c.value / 6) / 10, installs: c.installs });
  return {
    last,
    first,
    days: days.map((r) => ({
      day: r.day ?? "",
      reporting: own(r, "reporting"),
      active: own(r, "active"),
      agents: cell(r, "agents"),
      byUser: cell(r, "by_user"),
      byMain: cell(r, "by_main"),
      byAgent: cell(r, "by_agent"),
      agentHours: hours(cell(r, "agent_work")),
      crashes: cell(r, "crashes"),
    })),
    wau: own(win, "wau"),
    mau: own(win, "mau"),
    agentHours7: hours(cell(win, "agent_work")),
    focusedHours7: hours(cell(win, "focused")),
    versions: versions.map((r) => ({ version: r.version ?? "", channel: r.channel ?? "", installs: num(r.installs) })),
    surfaces: surfaces.map((r) => ({ surface: r.surface ?? "", installs: num(r.installs) })),
    quality: quality.map((r) => ({
      version: r.version ?? "",
      installs: num(r.installs),
      installDays: num(r.install_days),
      crashFreeDays: num(r.crash_free_days),
      crashes: num(r.crashes),
    })),
    attention: { open: cell(attention, "open", "n"), focused: cell(attention, "focused", "n"), engaged: cell(attention, "engaged", "n") },
    setup: {
      started: own(setup, "started"),
      done: own(setup, "done"),
      failedAuth: own(setup, "failed_auth"),
      failedNetwork: own(setup, "failed_network"),
      failedOther: own(setup, "failed_other"),
    },
  };
}
