// Writer's markdown bundle, office-md.js in its view's folder (md-artifact, architect m_38679;
// built by office/sdk/build.ts with its lockfile, architect m_60401): what draws a .md file in
// Writer (its markdown.js alone loads it). micromark with its GFM extension (MIT), the parser and
// the rules under the desktop thread's react-markdown + remark-gfm (same versions as apps/desktop's
// lockfile), so a document reads as an answer does. Its own bundle: it runs anywhere (node tests).

import { micromark } from "micromark";
import { gfm, gfmHtml } from "micromark-extension-gfm";

/**
 * A markdown text as HTML, GFM (tables, task lists, strikethrough, autolinks, footnotes). Safe
 * to put in a page: raw HTML in the text is escaped, and a link or an image whose protocol is not
 * http(s), mailto, irc(s), xmpp or a relative path loses its url (micromark's defaults).
 */
export function markdownHtml(text: string): string {
  return micromark(text, { extensions: [gfm()], htmlExtensions: [gfmHtml()] });
}
