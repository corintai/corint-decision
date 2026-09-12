export interface LayoutEdge {
  id: string;
  source: string;
  target: string;
}
export interface LayoutNode {
  id: string;
  position: { x: number; y: number };
}

// Place merges after all predecessors, so a shortcut cannot pull later nodes up.
export function flowRanks(ids: string[], edges: LayoutEdge[]) {
  const known = new Set(ids);
  const outgoing = new Map(ids.map((id) => [id, [] as string[]]));
  const incoming = new Map(ids.map((id) => [id, 0]));
  for (const edge of edges)
    if (known.has(edge.source) && known.has(edge.target)) {
      outgoing.get(edge.source)!.push(edge.target);
      incoming.set(edge.target, incoming.get(edge.target)! + 1);
    }
  const ranks: Record<string, number> = Object.create(null);
  const queue = ids.filter((id) => incoming.get(id) === 0);
  for (const id of queue) ranks[id] = 0;
  for (let i = 0; i < queue.length; i++) {
    const id = queue[i];
    for (const target of outgoing.get(id)!) {
      ranks[target] = Math.max(ranks[target] ?? 0, ranks[id] + 1);
      incoming.set(target, incoming.get(target)! - 1);
      if (incoming.get(target) === 0) queue.push(target);
    }
  }
  // Invalid cyclic drafts must remain editable without an unbounded layout pass.
  let last = Math.max(0, ...Object.values(ranks));
  for (const id of ids) if (!queue.includes(id)) ranks[id] = ++last;
  if (known.has("end"))
    ranks.end =
      Math.max(0, ...ids.filter((id) => id !== "end").map((id) => ranks[id])) +
      1;
  return ranks;
}

export function bypassLanes(nodes: LayoutNode[], edges: LayoutEdge[]) {
  const positions = new Map(nodes.map((node) => [node.id, node.position]));
  const right = Math.max(...nodes.map((node) => node.position.x + 252));
  const jumps = edges
    .filter((edge) => {
      const source = positions.get(edge.source),
        target = positions.get(edge.target);
      return (
        source && target && (target.y - source.y > 300 || target.y <= source.y)
      );
    })
    .sort(
      (a, b) =>
        positions.get(a.source)!.y - positions.get(b.source)!.y ||
        a.id.localeCompare(b.id),
    );
  return new Map(
    jumps.map((edge, index) => [
      edge.id,
      {
        // Early exits use outer lanes; later exits cannot cross those vertical lines.
        laneX: right + 64 + (jumps.length - index - 1) * 32,
        arrivalOffset: 24 + index * 16,
      },
    ]),
  );
}

export function bypassPath(
  sourceX: number,
  sourceY: number,
  targetX: number,
  targetY: number,
  laneX: number,
  arrivalOffset: number,
) {
  const departureY = sourceY + 28;
  const arrivalY = targetY - arrivalOffset;
  const radius = 8;
  return `M ${sourceX} ${sourceY} L ${sourceX} ${departureY - radius} Q ${sourceX} ${departureY} ${sourceX + radius} ${departureY} L ${laneX - radius} ${departureY} Q ${laneX} ${departureY} ${laneX} ${departureY + (arrivalY > departureY ? radius : -radius)} L ${laneX} ${arrivalY + (arrivalY > departureY ? -radius : radius)} Q ${laneX} ${arrivalY} ${laneX - radius} ${arrivalY} L ${targetX + radius} ${arrivalY} Q ${targetX} ${arrivalY} ${targetX} ${arrivalY + radius} L ${targetX} ${targetY}`;
}
