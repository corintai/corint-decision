import test from "node:test";
import assert from "node:assert/strict";
import { conditionLabel } from "./conditionLabel";

test("edge conditions preserve nested Boolean grouping and full expressions", () => {
  assert.equal(
    conditionLabel({
      all: ["user.ip_country not in list.high_risk_countries"],
    }),
    "user.ip_country not in list.high_risk_countries",
  );
  assert.equal(
    conditionLabel({
      all: [
        { any: ["event.amount > 100", "event.vip == true"] },
        { not: ["event.blocked == true"] },
      ],
    }),
    "((event.amount > 100)\nOR (event.vip == true))\nAND (NOT (event.blocked == true))",
  );
  assert.equal(conditionLabel("event.a || event.b"), "event.a || event.b");
  assert.equal(conditionLabel(undefined), "未配置条件");
  assert.equal(
    conditionLabel({ all: [], other: "invalid" }),
    '{"all":[],"other":"invalid"}',
  );
});
