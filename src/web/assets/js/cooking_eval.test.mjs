import test from "node:test";
import assert from "node:assert/strict";
import {
  SCHEMA_VERSION,
  truthy,
  buildEnv,
  evalExpr,
  evaluateRecipes,
  pickUniform,
  simulate,
} from "./cooking_eval.js";

test("schema version is pinned to the Rust payload contract", () => {
  assert.equal(SCHEMA_VERSION, 1);
});

test("Lua truthiness treats zero as true and nil only as falsey", () => {
  assert.equal(truthy(0), true);
  assert.equal(truthy(0.5), true);
  assert.equal(truthy(""), true);
  assert.equal(truthy(false), false);
  assert.equal(truthy(null), false);
  assert.equal(truthy(undefined), false);
});

test("buildEnv accumulates repeated ingredients and sums tags", () => {
  const env = buildEnv(
    [
      { prefab: "berries", key: "berries", tags: { fruit: 0.5 } },
      { prefab: "berries", key: "berries", tags: { fruit: 0.5 } },
      { prefab: "honey", key: "honey", tags: { sweetener: 1 } },
    ],
    "cookpot",
  );
  assert.equal(env.vars.names.berries, 2);
  assert.equal(env.vars.tags.fruit, 1);
  assert.equal(env.vars.tags.sweetener, 1);
  assert.equal(env.cooker, "cookpot");
});

test("evaluates the real recipe AST shape with short-circuit semantics", () => {
  // (names.butterflywings or names.moonbutterflywings) and not tags.meat
  // and tags.veggie >= 0.5
  const ast = [
    "and",
    [
      "and",
      [
        "or",
        ["field", ["var", "names"], "butterflywings"],
        ["field", ["var", "names"], "moonbutterflywings"],
      ],
      ["not", ["field", ["var", "tags"], "meat"]],
    ],
    ["ge", ["field", ["var", "tags"], "veggie"], ["num", 0.5]],
  ];
  const env = buildEnv(
    [
      { key: "butterflywings", tags: {} },
      { key: "carrot", tags: { veggie: 1 } },
      { key: "carrot", tags: { veggie: 1 } },
      { key: "berries", tags: { fruit: 0.5 } },
    ],
    "cookpot",
  );
  assert.equal(truthy(evalExpr(ast, env)), true);

  const withMeat = buildEnv(
    [
      { key: "butterflywings", tags: {} },
      { key: "meat", tags: { meat: 1 } },
      { key: "carrot", tags: { veggie: 1 } },
      { key: "carrot", tags: { veggie: 1 } },
    ],
    "cookpot",
  );
  assert.equal(truthy(evalExpr(ast, withMeat)), false);
});

test("cooker exposes the selected cooker name to future test expressions", () => {
  assert.equal(evalExpr(["var", "cooker"], { cooker: "cookpot", vars: {} }), "cookpot");
  assert.equal(evalExpr(["eq", ["var", "cooker"], ["str", "cookpot"]], { cooker: "cookpot", vars: {} }), true);
});

test("missing fields are nil; {x or 0} arithmetic works", () => {
  const env = buildEnv([{ key: "berries", tags: { fruit: 0.5 } }], "cookpot");
  const missing = ["or", ["field", ["var", "names"], "kelp"], ["num", 0]];
  assert.equal(evalExpr(missing, env), 0);
  assert.equal(truthy(evalExpr(missing, env)), true);
  const sum = [
    "add",
    ["or", ["field", ["var", "names"], "kelp"], ["num", 0]],
    ["or", ["field", ["var", "names"], "berries"], ["num", 0]],
  ];
  assert.equal(evalExpr(sum, env), 1);
});

test("evaluateRecipes keeps all equal top priority recipes", () => {
  const data = {
    cookers: { cookpot: ["alpha", "beta", "gamma"] },
    recipes: {
      alpha: { name: "alpha", priority: 10, test: ["ge", ["field", ["var", "tags"], "fruit"], ["num", 2]] },
      beta: { name: "beta", priority: 10, test: ["ge", ["field", ["var", "tags"], "fruit"], ["num", 2]] },
      gamma: { name: "gamma", priority: 5, test: ["bool", true] },
    },
  };
  const selected = [
    { key: "berries", tags: { fruit: 1 } },
    { key: "berries", tags: { fruit: 1 } },
  ];
  const result = evaluateRecipes(data, "cookpot", selected);
  assert.deepEqual(result.top.map((r) => r.name), ["alpha", "beta"]);
  assert.equal(result.topPriority, 10);
});

test("equal-priority draw is uniform over every top candidate", () => {
  const top = [{ name: "a" }, { name: "b" }, { name: "c" }];
  assert.equal(pickUniform(top, () => 0), 0);
  assert.equal(pickUniform(top, () => 0.34), 1);
  assert.equal(pickUniform(top, () => 0.99), 2);
});

test("simulate returns a chosen candidate and all alternatives", () => {
  const data = {
    cookers: { cookpot: ["alpha", "beta"] },
    recipes: {
      alpha: { name: "alpha", priority: 1, test: ["bool", true] },
      beta: { name: "beta", priority: 1, test: ["bool", true] },
    },
  };
  const result = simulate(data, "cookpot", [], () => 0.75);
  assert.equal(result.top.length, 2);
  assert.equal(result.chosen.name, "beta");
});
