// Sheets · a CSV file (docs/plugin-apps.md step 4: csv opens in Sheets, rank default): its text
// as rows for a one-sheet workbook, and the grid's first sheet back as CSV text when he saves.
// RFC 4180: a field in double quotes may hold commas, quotes ("" for one) and line breaks; a
// line ends in LF or CRLF. The separator is the file's: a comma, else a semicolon or a tab when
// the first line has more of those (a European export). A field that reads as a number in its
// plain form ("42", "-3.5", never "007" or "1e3") is a number, so a sum works; anything else
// stays its text, so the file comes back as it was.
//
// Pure (plugins/sheets/test/csv.test.mjs): sepOf, parseCsv, cellOf, csvOf.

import type { UWorkbook } from "./~kit/office/v1/office.js";

/** The file's separator: a comma, else a semicolon or a tab when its first line has more. */
export function sepOf(text: string): "," | ";" | "\t" {
  const first = text.slice(0, text.search(/\r?\n|$/));
  const n = (c: string) => first.split(c).length - 1;
  const [comma, semi, tab] = [n(","), n(";"), n("\t")];
  if (tab > comma && tab >= semi) return "\t";
  if (semi > comma) return ";";
  return ",";
}

/** A CSV text as rows of fields (strings), its separator found by sepOf. */
export function parseCsv(text: string, sep = sepOf(text)): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let field = "";
  let quoted = false;
  let i = 0;
  const s = text.replace(/^\uFEFF/, "");
  while (i < s.length) {
    const c = s[i];
    if (quoted) {
      if (c === '"' && s[i + 1] === '"') {
        field += '"';
        i += 2;
        continue;
      }
      if (c === '"') quoted = false;
      else field += c;
      i++;
      continue;
    }
    if (c === '"' && field === "") quoted = true;
    else if (c === sep) {
      row.push(field);
      field = "";
    } else if (c === "\n" || c === "\r") {
      row.push(field);
      rows.push(row);
      row = [];
      field = "";
      if (c === "\r" && s[i + 1] === "\n") i++;
    } else field += c;
    i++;
  }
  if (field !== "" || row.length > 0) {
    row.push(field);
    rows.push(row);
  }
  return rows;
}

/** A field as a cell: a number when its plain form is one, else its text; "" is empty. */
export function cellOf(field: string): string | number | null {
  if (field === "") return null;
  if (/^-?(0|[1-9]\d*)(\.\d+)?$/.test(field) && String(Number(field)) === field) return Number(field);
  return field;
}

/** A field as CSV: quoted when it holds the separator, a quote or a line break. */
function quote(v: string, sep: string) {
  return v.includes(sep) || /["\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/** The workbook's first sheet as CSV text: its values (a formula's result), up to its last
 * row and column with a value, LF line ends, the file's separator. */
export function csvOf(book: Pick<UWorkbook, "sheetOrder" | "sheets">, sep: string): string {
  const sheet = book.sheets[book.sheetOrder[0]];
  if (!sheet) return "";
  const cells = sheet.cellData || {};
  let lastRow = -1;
  let lastCol = -1;
  for (const [r, cols] of Object.entries(cells)) {
    for (const [c, cell] of Object.entries(cols || {})) {
      if (cell && cell.v !== undefined && cell.v !== null && cell.v !== "") {
        lastRow = Math.max(lastRow, Number(r));
        lastCol = Math.max(lastCol, Number(c));
      }
    }
  }
  const lines: string[] = [];
  for (let r = 0; r <= lastRow; r++) {
    const fields: string[] = [];
    for (let c = 0; c <= lastCol; c++) {
      const v = cells[r]?.[c]?.v;
      fields.push(v === undefined || v === null ? "" : quote(String(v), sep));
    }
    lines.push(fields.join(sep));
  }
  return lines.length ? `${lines.join("\n")}\n` : "";
}
