// Tiny static server for previewing src/ pages (minibar.html, index.html) in a
// browser without running the Tauri shell. Pages fall back gracefully when
// window.__TAURI__ is absent; inject mock data via the console to style them.
const http = require("http");
const fs = require("fs");
const path = require("path");
const SRC = path.join(__dirname, "..", "src");
const MIME = { ".html": "text/html", ".js": "text/javascript", ".css": "text/css", ".png": "image/png", ".webp": "image/webp" };
http.createServer((req, res) => {
  const urlPath = decodeURIComponent(req.url.split("?")[0]);
  const file = path.join(SRC, urlPath === "/" ? "index.html" : urlPath);
  if (!path.resolve(file).startsWith(path.resolve(SRC))) { res.writeHead(403); return res.end(); }
  fs.readFile(file, (err, data) => {
    if (err) { res.writeHead(404); return res.end("not found"); }
    res.writeHead(200, { "Content-Type": MIME[path.extname(file)] || "application/octet-stream" });
    res.end(data);
  });
}).listen(4173, () => console.log("serving " + SRC + " on http://localhost:4173"));
