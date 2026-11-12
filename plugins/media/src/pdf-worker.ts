// pdf.js's worker (pdfjs-dist, Apache-2.0) as office/dist/pdf.worker.js: kit/office/pdf.js
// starts it as a module worker from /kit/office/pdf.worker.js (architect m_24668: a file
// served under the viewer's script rule, never a blob). The worker sets itself up on load.

// @ts-expect-error pdfjs-dist ships no types for its worker module
export * from "pdfjs-dist/build/pdf.worker.mjs";
