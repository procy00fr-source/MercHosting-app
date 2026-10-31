// Extrae textos en español de los .tsx/.ts de la interfaz (literales, JSX y plantillas).
// Necesita la API de TypeScript 5 (la 7 del proyecto no la tiene):
//   npm i --prefix /tmp/ts5 typescript@5 && TS5=/tmp/ts5/node_modules/typescript node scripts/i18n/extract-ui.cjs
const ts = require(process.env.TS5 || "typescript");
const fs = require("fs"), path = require("path");
const root = require("path").resolve(__dirname, "../../app/src");
const files = [];
(function walk(d) {
  for (const f of fs.readdirSync(d)) {
    const p = path.join(d, f);
    if (fs.statSync(p).isDirectory()) { if (!["test"].includes(f)) walk(p); }
    else if (/\.(tsx?|)$/.test(f) && !/\.test\.tsx?$/.test(f) && !/\.d\.ts$/.test(f)) files.push(p);
  }
})(root);
const exact = new Map(), patterns = new Map();
const spanish = (s) => /[a-záéíóúñ]{2,}/i.test(s) && !/^[a-z0-9_.:\/\-#@%]+$/.test(s.trim()) ;
const skipKeys = /^(className|key|id|htmlFor|type|role|href|src|op|kind|name|value|data-|autoComplete|inputMode|rel|target|method|lang|d|fill|stroke|viewBox|xmlns|loading|wrap)$/;
function add(s, file) {
  const t = s.replace(/\s+/g, " ").trim();
  if (t.length < 2 || !spanish(t)) return;
  if (!/[a-zA-Z]/.test(t)) return;
  exact.set(t, (exact.get(t) || new Set()).add(path.relative(root, file)));
}
function addPat(parts, file) {
  // parts: [literal, literal, ...] with placeholders between
  const lits = parts.map((p) => p.replace(/\s+/g, " "));
  const joined = lits.join("");
  if (!spanish(joined)) return;
  let key = lits[0];
  for (let i = 1; i < lits.length; i++) key += `{${i - 1}}` + lits[i];
  key = key.trim();
  if (!/[a-záéíóúñ]{2,}/i.test(lits.join(" "))) return;
  patterns.set(key, (patterns.get(key) || new Set()).add(path.relative(root, file)));
}
for (const file of files) {
  const src = fs.readFileSync(file, "utf8");
  const sf = ts.createSourceFile(file, src, ts.ScriptTarget.Latest, true, file.endsWith("x") ? ts.ScriptKind.TSX : ts.ScriptKind.TS);
  function visit(n) {
    if (ts.isImportDeclaration(n) || ts.isExportDeclaration(n)) return;
    if (ts.isStringLiteral(n) || ts.isNoSubstitutionTemplateLiteral(n)) {
      const p = n.parent;
      if (p && ts.isJsxAttribute(p) && skipKeys.test(p.name.getText())) return;
      if (p && (ts.isPropertyAssignment(p) && p.name === n)) return;
      if (p && ts.isElementAccessExpression(p)) return;
      if (p && ts.isLiteralTypeNode(p)) return;
      if (p && ts.isCallExpression(p) && /^(require|querySelector|getItem|setItem|matchMedia|addEventListener|includes|startsWith|endsWith|split|replace|test|invoke|call)$/.test((p.expression.name || p.expression).getText?.() || "")) {}
      // comparisons like x === "algo"
      if (p && ts.isBinaryExpression(p) && [ts.SyntaxKind.EqualsEqualsEqualsToken, ts.SyntaxKind.ExclamationEqualsEqualsToken].includes(p.operatorToken.kind)) return;
      if (p && ts.isCaseClause(p)) return;
      add(n.text, file);
    } else if (ts.isTemplateExpression(n)) {
      const parts = [n.head.text, ...n.templateSpans.map((s) => s.literal.text)];
      addPat(parts, file);
    } else if (ts.isJsxText(n)) {
      add(n.text, file);
    } else if (ts.isJsxElement(n) || ts.isJsxFragment(n)) {
      // Texto JSX mezclado con expresiones: también como patrón.
      const kids = n.children;
      const hasText = kids.some((k) => ts.isJsxText(k) && k.text.trim());
      const hasExpr = kids.some((k) => ts.isJsxExpression(k));
      const onlyTextAndExpr = kids.every((k) => ts.isJsxText(k) || ts.isJsxExpression(k));
      if (hasText && hasExpr && onlyTextAndExpr) {
        const parts = [""];
        for (const k of kids) {
          if (ts.isJsxText(k)) parts[parts.length - 1] += k.text;
          else if (k.expression && (ts.isStringLiteral(k.expression))) parts[parts.length - 1] += k.expression.text;
          else parts.push("");
        }
        addPat(parts, file);
      }
    }
    ts.forEachChild(n, visit);
  }
  visit(sf);
}
const out = {
  exact: [...exact.entries()].map(([k, v]) => ({ k, f: [...v] })),
  patterns: [...patterns.entries()].map(([k, v]) => ({ k, f: [...v] })),
};
fs.writeFileSync(process.argv[2] || "i18n-ui.json", JSON.stringify(out, null, 1));
console.log("exact", out.exact.length, "patterns", out.patterns.length, "chars", out.exact.reduce((a, b) => a + b.k.length, 0) + out.patterns.reduce((a, b) => a + b.k.length, 0));
