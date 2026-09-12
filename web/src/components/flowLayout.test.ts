import test from "node:test";
import assert from "node:assert/strict";
import { flowRanks, bypassLanes, bypassPath } from "./flowLayout";

test("merge nodes stay below their longest branch and cyclic drafts terminate", () => {
  const edges = [
    { id: "1", source: "__entry", target: "router" },
    { id: "2", source: "router", target: "merge" },
    { id: "3", source: "router", target: "check" },
    { id: "4", source: "check", target: "merge" },
    { id: "5", source: "merge", target: "end" },
  ];
  const ranks = flowRanks(
    ["__entry", "router", "check", "merge", "end"],
    edges,
  );
  for (const edge of edges) assert.ok(ranks[edge.target] > ranks[edge.source]);
  const cyclic = flowRanks(
    ["a", "b", "end"],
    [
      { id: "a", source: "a", target: "b" },
      { id: "b", source: "b", target: "a" },
    ],
  );
  assert.ok(Object.values(cyclic).every(Number.isFinite));
});

test("early exits use separate outer lanes with staggered arrivals", () => {
  const nodes = [
    { id: "a", position: { x: 0, y: 0 } },
    { id: "b", position: { x: 0, y: 240 } },
    { id: "c", position: { x: 190, y: 480 } },
    { id: "end", position: { x: 0, y: 960 } },
  ];
  const lanes = bypassLanes(nodes, [
    { id: "ab", source: "a", target: "b" },
    { id: "ae", source: "a", target: "end" },
    { id: "be", source: "b", target: "end" },
  ]);
  assert.equal(lanes.has("ab"), false);
  assert.ok(lanes.get("ae")!.laneX > lanes.get("be")!.laneX);
  assert.ok(lanes.get("be")!.laneX > 190 + 252);
  assert.ok(lanes.get("ae")!.arrivalOffset < lanes.get("be")!.arrivalOffset);
  const path = bypassPath(168, 110, 126, 960, lanes.get("ae")!.laneX, 24);
  assert.ok(path.startsWith("M 168 110"));
  assert.ok(path.endsWith("L 126 960"));
});
