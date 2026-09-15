//! Cooking simulator interpreter.
//!
//! The Rust backend compiles DST recipe `test` functions into the compact AST
//! consumed here. Keeping the interpreter in the browser makes the data
//! payload reusable for a future static wiki gadget; the Node test file next
//! to this module pins the small set of Lua semantics that matter.

export const SCHEMA_VERSION = 1;

/** Lua truthiness: only nil/false are falsey; 0 is truthy. */
export function truthy(value) {
  return value !== null && value !== undefined && value !== false;
}

/**
 * Build the evaluation environment for four selected ingredient payloads.
 * Each item is an entry from `CookingData.ingredients`.
 */
export function buildEnv(selected, cooker) {
  const names = Object.create(null);
  const tags = Object.create(null);
  for (const item of selected) {
    const key = item.key || item.prefab;
    names[key] = (names[key] || 0) + 1;
    for (const [tag, value] of Object.entries(item.tags || {})) {
      tags[tag] = (tags[tag] || 0) + value;
    }
  }
  return { vars: { names, tags }, cooker };
}

function readField(base, name) {
  if (base === null || base === undefined) return undefined;
  return base[name];
}

function numberOperand(value, op) {
  if (typeof value !== "number" || Number.isNaN(value)) {
    throw new Error(`cooking_eval: ${op} requires numeric operands`);
  }
  return value;
}

/** Evaluate one compiled recipe AST node. Throws on malformed/unsupported nodes. */
export function evalExpr(expr, env) {
  if (!Array.isArray(expr) || expr.length === 0) {
    throw new Error("cooking_eval: malformed AST node");
  }
  const [op] = expr;
  switch (op) {
    case "nil":
      return null;
    case "bool":
      return !!expr[1];
    case "num":
      return Number(expr[1]);
    case "str":
      return String(expr[1]);
    case "var":
      return env.vars[expr[1]];
    case "field":
      return readField(evalExpr(expr[1], env), String(expr[2]));
    case "not":
      return !truthy(evalExpr(expr[1], env));
    case "neg":
      return -numberOperand(evalExpr(expr[1], env), "neg");
    case "and": {
      const lhs = evalExpr(expr[1], env);
      return truthy(lhs) ? evalExpr(expr[2], env) : lhs;
    }
    case "or": {
      const lhs = evalExpr(expr[1], env);
      return truthy(lhs) ? lhs : evalExpr(expr[2], env);
    }
    case "add": {
      return (
        numberOperand(evalExpr(expr[1], env), "add") +
        numberOperand(evalExpr(expr[2], env), "add")
      );
    }
    case "sub": {
      return (
        numberOperand(evalExpr(expr[1], env), "sub") -
        numberOperand(evalExpr(expr[2], env), "sub")
      );
    }
    case "mul": {
      return (
        numberOperand(evalExpr(expr[1], env), "mul") *
        numberOperand(evalExpr(expr[2], env), "mul")
      );
    }
    case "div": {
      return (
        numberOperand(evalExpr(expr[1], env), "div") /
        numberOperand(evalExpr(expr[2], env), "div")
      );
    }
    case "eq": {
      const a = evalExpr(expr[1], env);
      const b = evalExpr(expr[2], env);
      if (a === null || a === undefined || b === null || b === undefined) {
        return (a === null || a === undefined) && (b === null || b === undefined);
      }
      return a === b;
    }
    case "ne":
      return !evalExpr(["eq", expr[1], expr[2]], env);
    case "gt":
      return numberOperand(evalExpr(expr[1], env), "gt") > numberOperand(evalExpr(expr[2], env), "gt");
    case "ge":
      return numberOperand(evalExpr(expr[1], env), "ge") >= numberOperand(evalExpr(expr[2], env), "ge");
    case "lt":
      return numberOperand(evalExpr(expr[1], env), "lt") < numberOperand(evalExpr(expr[2], env), "lt");
    case "le":
      return numberOperand(evalExpr(expr[1], env), "le") <= numberOperand(evalExpr(expr[2], env), "le");
    default:
      throw new Error(`cooking_eval: unsupported AST opcode ${String(op)}`);
  }
}

/**
 * Evaluate all recipes available to `cooker`.
 *
 * Returns `{matches, top, topPriority}` where `top` is the equal-priority set
 * shown in the UI. All current DST weights are 1; the `weight` field remains
 * in the payload for a future weighted draw.
 */
export function evaluateRecipes(data, cooker, selected) {
  const names = data.cookers?.[cooker] || [];
  const env = buildEnv(selected, cooker);
  const matches = [];
  for (const name of names) {
    const recipe = data.recipes?.[name];
    if (!recipe || !truthy(evalExpr(recipe.test, env))) continue;
    matches.push(recipe);
  }
  if (matches.length === 0) {
    return { matches: [], top: [], topPriority: null };
  }
  const topPriority = Math.max(...matches.map((r) => Number(r.priority) || 0));
  const top = matches
    .filter((r) => (Number(r.priority) || 0) === topPriority)
    .sort((a, b) => a.name.localeCompare(b.name));
  return { matches, top, topPriority };
}

/** Uniform pick used while all recipe weights are 1. */
export function pickUniform(top, rng = Math.random) {
  if (!top || top.length === 0) return -1;
  const roll = Number(rng());
  const index = Math.floor(roll * top.length);
  return Math.min(Math.max(index, 0), top.length - 1);
}

/** Convenience wrapper for the UI: evaluate then roll one top candidate. */
export function simulate(data, cooker, selected, rng = Math.random) {
  const result = evaluateRecipes(data, cooker, selected);
  const chosenIndex = pickUniform(result.top, rng);
  return { ...result, chosenIndex, chosen: result.top[chosenIndex] || null };
}
