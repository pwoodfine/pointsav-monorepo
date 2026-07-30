/**
 * Workplace*Proforma — engine.js
 * Phase 1 MVP formula engine.
 *
 * Copyright © 2026 PointSav Digital Systems — EUPL-1.2
 *
 * This is a minimal JavaScript formula engine sufficient for the MVP scope.
 * Supported:
 *   - Arithmetic: + - * / ^ ( )
 *   - Cell references: A1, B2, $A$1 (A1-notation, absolute/relative)
 *   - Range references: A1:B5 (returned as flat array)
 *   - Semantic references: section.line_id.yN (dotted identifier path)
 *   - Functions: SUM, AVERAGE, MIN, MAX, COUNT, IF, ROUND, ABS, NEG
 *     Financial: PMT, PV, FV, NPV, IRR
 *   - String comparison operators: = <> > < >= <=
 *
 * Phase 2 replaces this module with a Rust-side IronCalc (or Formualizer)
 * engine invoked via Tauri IPC. The public API of this module — evaluate(),
 * parseRef(), getCell(), setCell() — is designed to be stable across that
 * transition so the grid and toolbar code does not change.
 */

'use strict';

window.WorkplaceEngine = (function () {

  /* ─── Cell storage ──────────────────────────────────────────────────── */

  // Current active sheet's cells: { "A1": { raw, value, formula, format }, ... }
  let cells = {};

  // Semantic id → A1 coordinate mapping (for formulas that reference line ids)
  let idToCoord = {};

  // Column headers currently in play: ['Y1', 'Y2', 'Y3', ...]
  let yearColumns = [];

  /* ─── A1 notation ────────────────────────────────────────────────────── */

  function colIndexToLetter(idx) {
    let s = '';
    while (idx >= 0) {
      s = String.fromCharCode(65 + (idx % 26)) + s;
      idx = Math.floor(idx / 26) - 1;
    }
    return s;
  }

  function colLetterToIndex(letters) {
    let idx = 0;
    for (let i = 0; i < letters.length; i++) {
      idx = idx * 26 + (letters.charCodeAt(i) - 64);
    }
    return idx - 1;
  }

  function parseRef(ref) {
    // Strip absolute markers ($)
    const clean = ref.replace(/\$/g, '').toUpperCase();
    const m = clean.match(/^([A-Z]+)(\d+)$/);
    if (!m) return null;
    return { col: colLetterToIndex(m[1]), row: parseInt(m[2], 10) - 1, ref: clean };
  }

  function coord(col, row) {
    return colIndexToLetter(col) + (row + 1);
  }

  /* ─── Cell accessors ─────────────────────────────────────────────────── */

  function setCell(ref, raw) {
    const trimmed = String(raw).trim();
    if (trimmed === '') {
      delete cells[ref];
      return;
    }
    const cell = { raw: trimmed };
    if (trimmed.startsWith('=')) {
      cell.formula = trimmed.substring(1);
      cell.value = null;  // evaluated later
    } else if (!isNaN(parseFloat(trimmed)) && isFinite(trimmed)) {
      cell.value = parseFloat(trimmed);
    } else {
      cell.value = trimmed;
    }
    cells[ref] = cell;
  }

  function getCell(ref) {
    return cells[ref] || null;
  }

  function getValue(ref) {
    const cell = cells[ref];
    if (!cell) return 0;  // empty cells are zero in arithmetic
    return cell.value;
  }

  function setCells(newCells) {
    cells = newCells || {};
  }

  function getAllCells() {
    return cells;
  }

  function setIdMap(map) {
    idToCoord = map || {};
  }

  function setYearColumns(cols) {
    yearColumns = cols || [];
  }

  /* ─── Tokeniser ──────────────────────────────────────────────────────── */

  function tokenise(expr) {
    const tokens = [];
    let i = 0;
    while (i < expr.length) {
      const c = expr[i];
      if (c === ' ' || c === '\t' || c === '\n') { i++; continue; }

      // String literal
      if (c === '"') {
        let j = i + 1;
        while (j < expr.length && expr[j] !== '"') j++;
        tokens.push({ t: 'str', v: expr.substring(i + 1, j) });
        i = j + 1; continue;
      }

      // Number
      if ((c >= '0' && c <= '9') || (c === '.' && expr[i + 1] >= '0' && expr[i + 1] <= '9')) {
        let j = i;
        while (j < expr.length && ((expr[j] >= '0' && expr[j] <= '9') || expr[j] === '.')) j++;
        tokens.push({ t: 'num', v: parseFloat(expr.substring(i, j)) });
        i = j; continue;
      }

      // Identifier / reference / function name
      if ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c === '_' || c === '$') {
        let j = i;
        while (j < expr.length && /[A-Za-z0-9_$.:]/.test(expr[j])) j++;
        const raw = expr.substring(i, j);
        i = j;

        // Is it followed by ( ? Then it is a function call
        while (i < expr.length && expr[i] === ' ') i++;
        if (expr[i] === '(') {
          tokens.push({ t: 'fn', v: raw.toUpperCase() });
          continue;
        }

        // Is it a range (contains colon) ?
        if (raw.includes(':')) {
          tokens.push({ t: 'range', v: raw });
          continue;
        }

        // A1-notation?
        if (/^\$?[A-Za-z]+\$?\d+$/.test(raw)) {
          tokens.push({ t: 'ref', v: raw });
          continue;
        }

        // Boolean constants
        if (raw.toUpperCase() === 'TRUE')  { tokens.push({ t: 'num', v: 1 }); continue; }
        if (raw.toUpperCase() === 'FALSE') { tokens.push({ t: 'num', v: 0 }); continue; }

        // Semantic identifier (e.g. pgi, revenue.pgi, revenue.pgi.y1)
        tokens.push({ t: 'id', v: raw });
        continue;
      }

      // Two-character operators
      const two = expr.substring(i, i + 2);
      if (two === '<=' || two === '>=' || two === '<>') {
        tokens.push({ t: 'op', v: two });
        i += 2; continue;
      }

      // Single-character operators
      if ('+-*/^()=<>,'.indexOf(c) >= 0) {
        tokens.push({ t: 'op', v: c });
        i++; continue;
      }

      // Unknown character — skip
      i++;
    }
    return tokens;
  }

  /* ─── Parser (Pratt / recursive descent) ─────────────────────────────── */

  function parse(tokens) {
    let pos = 0;

    function peek() { return tokens[pos]; }
    function next() { return tokens[pos++]; }
    function expect(t, v) {
      const tok = next();
      if (!tok || tok.t !== t || (v !== undefined && tok.v !== v)) {
        throw new Error(`Expected ${v || t}, got ${tok ? tok.v : 'end of formula'}`);
      }
      return tok;
    }

    // Precedence:  < =  +  *  ^  unary  primary
    function parseExpr() {
      return parseComparison();
    }

    function parseComparison() {
      let left = parseAddSub();
      while (peek() && peek().t === 'op' && ['=', '<>', '<', '>', '<=', '>='].indexOf(peek().v) >= 0) {
        const op = next().v;
        const right = parseAddSub();
        left = { type: 'binop', op, left, right };
      }
      return left;
    }

    function parseAddSub() {
      let left = parseMulDiv();
      while (peek() && peek().t === 'op' && (peek().v === '+' || peek().v === '-')) {
        const op = next().v;
        const right = parseMulDiv();
        left = { type: 'binop', op, left, right };
      }
      return left;
    }

    function parseMulDiv() {
      let left = parsePow();
      while (peek() && peek().t === 'op' && (peek().v === '*' || peek().v === '/')) {
        const op = next().v;
        const right = parsePow();
        left = { type: 'binop', op, left, right };
      }
      return left;
    }

    function parsePow() {
      let left = parseUnary();
      if (peek() && peek().t === 'op' && peek().v === '^') {
        next();
        const right = parsePow();  // right-associative
        return { type: 'binop', op: '^', left, right };
      }
      return left;
    }

    function parseUnary() {
      if (peek() && peek().t === 'op' && (peek().v === '-' || peek().v === '+')) {
        const op = next().v;
        const operand = parseUnary();
        return { type: 'unary', op, operand };
      }
      return parsePrimary();
    }

    function parsePrimary() {
      const tok = next();
      if (!tok) throw new Error('Unexpected end of formula');

      if (tok.t === 'num') return { type: 'num', value: tok.v };
      if (tok.t === 'str') return { type: 'str', value: tok.v };
      if (tok.t === 'ref') return { type: 'ref', ref: tok.v.replace(/\$/g, '').toUpperCase() };
      if (tok.t === 'range') {
        const [a, b] = tok.v.split(':');
        return { type: 'range', a: a.replace(/\$/g, '').toUpperCase(), b: b.replace(/\$/g, '').toUpperCase() };
      }
      if (tok.t === 'id') return { type: 'id', name: tok.v };

      if (tok.t === 'fn') {
        expect('op', '(');
        const args = [];
        if (!(peek() && peek().t === 'op' && peek().v === ')')) {
          args.push(parseExpr());
          while (peek() && peek().t === 'op' && peek().v === ',') {
            next();
            args.push(parseExpr());
          }
        }
        expect('op', ')');
        return { type: 'fn', name: tok.v, args };
      }

      if (tok.t === 'op' && tok.v === '(') {
        const inner = parseExpr();
        expect('op', ')');
        return inner;
      }

      throw new Error(`Unexpected token: ${tok.v}`);
    }

    return parseExpr();
  }

  /* ─── Evaluator ──────────────────────────────────────────────────────── */

  function resolveId(name, contextCoord) {
    // Direct lookup in the id map (produced by the grid)
    if (idToCoord[name]) return idToCoord[name];

    // Dotted path — last segment may be year reference (y1, y2...)
    const parts = name.split('.');
    const last = parts[parts.length - 1];
    const rest = parts.slice(0, -1).join('.');

    if (/^y\d+$/i.test(last) && idToCoord[rest]) {
      const baseCoord = parseRef(idToCoord[rest]);
      const yearIdx = parseInt(last.substring(1), 10) - 1;
      // Find the column for yN in the currently active sheet
      const yearCol = yearColumns.indexOf(last.toLowerCase());
      if (yearCol >= 0 && baseCoord) {
        // The coordinate system: label col = 0, first year col starts after
        // the frozen columns. The caller's idToCoord tells us the row; we
        // override the column with the year's position.
        return coord(yearCol, baseCoord.row);
      }
    }

    return null;
  }

  function rangeCoords(a, b) {
    const ra = parseRef(a);
    const rb = parseRef(b);
    if (!ra || !rb) return [];
    const coords = [];
    const c0 = Math.min(ra.col, rb.col);
    const c1 = Math.max(ra.col, rb.col);
    const r0 = Math.min(ra.row, rb.row);
    const r1 = Math.max(ra.row, rb.row);
    for (let r = r0; r <= r1; r++) {
      for (let c = c0; c <= c1; c++) {
        coords.push(coord(c, r));
      }
    }
    return coords;
  }

  function numericValue(v) {
    if (v === null || v === undefined || v === '') return 0;
    if (typeof v === 'number') return v;
    if (typeof v === 'boolean') return v ? 1 : 0;
    const n = parseFloat(v);
    return isNaN(n) ? 0 : n;
  }

  function isError(v) {
    return typeof v === 'string' && v.charAt(0) === '#' && v.charAt(v.length - 1) === '!';
  }

  function firstError(values) {
    for (const v of values) {
      if (isError(v)) return v;
      if (Array.isArray(v)) {
        const inner = firstError(v);
        if (inner) return inner;
      }
    }
    return null;
  }

  function evalNode(node, visited, contextCoord) {
    if (!node) return 0;

    switch (node.type) {
      case 'num': return node.value;
      case 'str': return node.value;

      case 'unary': {
        const v = evalNode(node.operand, visited, contextCoord);
        if (isError(v)) return v;
        if (node.op === '-') return -numericValue(v);
        return numericValue(v);
      }

      case 'binop': {
        const l = evalNode(node.left, visited, contextCoord);
        if (isError(l)) return l;
        const r = evalNode(node.right, visited, contextCoord);
        if (isError(r)) return r;
        const ln = numericValue(l);
        const rn = numericValue(r);
        switch (node.op) {
          case '+': return ln + rn;
          case '-': return ln - rn;
          case '*': return ln * rn;
          case '/': return rn === 0 ? '#DIV/0!' : ln / rn;
          case '^': return Math.pow(ln, rn);
          case '=': return l == r ? 1 : 0;
          case '<>': return l != r ? 1 : 0;
          case '<': return ln < rn ? 1 : 0;
          case '>': return ln > rn ? 1 : 0;
          case '<=': return ln <= rn ? 1 : 0;
          case '>=': return ln >= rn ? 1 : 0;
        }
        return 0;
      }

      case 'ref': {
        if (visited.has(node.ref)) return '#CIRC!';
        return evalCellByRef(node.ref, visited);
      }

      case 'id': {
        const resolved = resolveId(node.name, contextCoord);
        if (!resolved) return '#NAME?';
        if (visited.has(resolved)) return '#CIRC!';
        return evalCellByRef(resolved, visited);
      }

      case 'range': {
        return rangeCoords(node.a, node.b).map(c => evalCellByRef(c, visited));
      }

      case 'fn': {
        const args = node.args.map(a => evalNode(a, visited, contextCoord));
        // Error propagation: if any argument is an error, the function
        // returns that error (matches Excel behaviour). Exception: IF can
        // have an error in the branch that is not taken — handled inside.
        if (node.name !== 'IF') {
          const err = firstError(args);
          if (err) return err;
        }
        return evalFn(node.name, args);
      }
    }
    return 0;
  }

  function flattenArgs(args) {
    const out = [];
    for (const a of args) {
      if (Array.isArray(a)) out.push(...a);
      else out.push(a);
    }
    return out;
  }

  function evalFn(name, args) {
    const flat = flattenArgs(args);
    const nums = flat.map(numericValue);

    switch (name) {
      case 'SUM':     return nums.reduce((a, b) => a + b, 0);
      case 'AVERAGE': return nums.length ? nums.reduce((a, b) => a + b, 0) / nums.length : 0;
      case 'MIN':     return nums.length ? Math.min(...nums) : 0;
      case 'MAX':     return nums.length ? Math.max(...nums) : 0;
      case 'COUNT':   return flat.filter(v => typeof v === 'number' && !isNaN(v)).length;
      case 'ABS':     return Math.abs(nums[0] || 0);
      case 'ROUND':   {
        const [v, d] = nums;
        const mult = Math.pow(10, d || 0);
        return Math.round((v || 0) * mult) / mult;
      }
      case 'IF': {
        const [cond, tVal, fVal] = args;
        return numericValue(cond) ? tVal : fVal;
      }
      case 'NEG': return -(nums[0] || 0);

      // Financial
      case 'PMT': {
        // PMT(rate, nper, pv) — rate per period, periods, present value
        const [rate, nper, pv] = nums;
        if (rate === 0) return -pv / nper;
        return (-pv * rate) / (1 - Math.pow(1 + rate, -nper));
      }
      case 'PV': {
        const [rate, nper, pmt] = nums;
        if (rate === 0) return -pmt * nper;
        return -pmt * (1 - Math.pow(1 + rate, -nper)) / rate;
      }
      case 'FV': {
        const [rate, nper, pmt, pv] = nums;
        const pv0 = pv || 0;
        if (rate === 0) return -(pv0 + pmt * nper);
        return -(pv0 * Math.pow(1 + rate, nper) + pmt * (Math.pow(1 + rate, nper) - 1) / rate);
      }
      case 'NPV': {
        // NPV(rate, v1, v2, v3...)
        const rate = nums[0];
        let sum = 0;
        for (let i = 1; i < nums.length; i++) {
          sum += nums[i] / Math.pow(1 + rate, i);
        }
        return sum;
      }
      case 'IRR': {
        // IRR via Newton-Raphson
        const cashflows = nums;
        let guess = 0.1;
        for (let iter = 0; iter < 50; iter++) {
          let npv = 0, dnpv = 0;
          for (let t = 0; t < cashflows.length; t++) {
            npv  += cashflows[t] / Math.pow(1 + guess, t);
            dnpv -= t * cashflows[t] / Math.pow(1 + guess, t + 1);
          }
          if (Math.abs(npv) < 1e-7) return guess;
          if (dnpv === 0) break;
          guess = guess - npv / dnpv;
        }
        return '#NUM!';
      }
    }

    return '#NAME?';
  }

  /* ─── Cell evaluation ────────────────────────────────────────────────── */

  function evalCellByRef(ref, visited) {
    const cell = cells[ref];
    if (!cell) return 0;

    // Non-formula: return stored value directly
    if (!cell.formula) return cell.value;

    // Memoise — already computed this pass
    if ('computed' in cell) return cell.computed;

    // Cycle detection
    visited.add(ref);
    try {
      const tokens = tokenise(cell.formula);
      const ast = parse(tokens);
      const result = evalNode(ast, visited, ref);
      cell.computed = result;
      return result;
    } catch (err) {
      cell.computed = '#ERR!';
      return '#ERR!';
    } finally {
      visited.delete(ref);
    }
  }

  /**
   * Evaluate every formula cell in the workbook. Results are written back
   * into each cell's `value` property so the grid can read them directly.
   * Sub-millisecond for typical proforma workloads in the MVP scope.
   */
  function evaluateAll() {
    // Clear memoisation from previous pass
    for (const ref in cells) {
      if (cells[ref].formula) delete cells[ref].computed;
    }

    // Evaluate every formula cell
    for (const ref in cells) {
      if (cells[ref].formula) {
        cells[ref].value = evalCellByRef(ref, new Set());
      }
    }
  }

  /**
   * Evaluate a standalone formula string and return the result.
   * Used by the formula bar to show live previews.
   */
  function evaluateFormula(formula, contextCoord) {
    try {
      const expr = formula.startsWith('=') ? formula.substring(1) : formula;
      const tokens = tokenise(expr);
      const ast = parse(tokens);
      return evalNode(ast, new Set(), contextCoord);
    } catch (err) {
      return '#ERR!';
    }
  }

  /* ─── Formatting ─────────────────────────────────────────────────────── */

  function formatValue(value, fmt) {
    if (value === null || value === undefined) return '';
    if (typeof value === 'string' && value.startsWith('#')) return value;  // errors

    if (typeof value !== 'number' || isNaN(value)) return String(value);

    const isNeg = value < 0;
    const abs = Math.abs(value);

    // Thousand-separated integer component
    function withCommas(n, dp) {
      return n.toLocaleString('en-US', {
        minimumFractionDigits: dp,
        maximumFractionDigits: dp,
      });
    }

    switch (fmt) {
      case 'currency-0dp':   return isNeg ? '(' + withCommas(abs, 0) + ')' : withCommas(abs, 0);
      case 'currency-2dp':   return isNeg ? '(' + withCommas(abs, 2) + ')' : withCommas(abs, 2);
      case 'number-0dp':     return withCommas(value, 0);
      case 'number-2dp':     return withCommas(value, 2);
      case 'percent-0dp':    return (value * 100).toFixed(0) + '%';
      case 'percent-1dp':    return (value * 100).toFixed(1) + '%';
      case 'percent-2dp':    return (value * 100).toFixed(2) + '%';
      case 'ratio-2dp':      return value.toFixed(2) + 'x';
      default:               return withCommas(value, 2);
    }
  }

  return {
    setCell,
    getCell,
    getValue,
    setCells,
    getAllCells,
    setIdMap,
    setYearColumns,
    evaluateAll,
    evaluateFormula,
    formatValue,
    parseRef,
    coord,
    colIndexToLetter,
    colLetterToIndex,
  };

})();
