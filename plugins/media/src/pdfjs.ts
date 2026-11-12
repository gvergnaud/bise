// The viewer's pdf bundle, office/dist/office-pdf.js (pdf-notes, architect m_24668): pdf.js
// (pdfjs-dist, Apache-2.0), what draws a pdf's pages and their text layer into the viewer.
// Loaded only by kit/office/pdf.js; its worker is the next bundle, office/dist/pdf.worker.js,
// served beside it under the same script rule.

export { getDocument, GlobalWorkerOptions, TextLayer, Util } from "pdfjs-dist";
