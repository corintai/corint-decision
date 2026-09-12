import { useMemo, useState } from "react";
import {
  Background,
  Controls,
  Handle,
  MarkerType,
  MiniMap,
  Position,
  Panel,
  ReactFlow,
  useReactFlow,
  type Connection,
  type Edge,
  type Node,
  type NodeProps,
  type XYPosition,
} from "@xyflow/react";
import {
  ArrowRight,
  Braces,
  GitBranch,
  Layers,
  Play,
  ShieldCheck,
  Webhook,
} from "lucide-react";
import { object, type ObjectValue, type Step } from "../model";
import ConditionEdge from "./ConditionEdge";
import { conditionLabel } from "./conditionLabel";
import { flowRanks, bypassLanes } from "./flowLayout";

const icons = {
  rule: ShieldCheck,
  ruleset: Layers,
  router: GitBranch,
  pipeline: Braces,
  service: Webhook,
  start: Play,
  end: ShieldCheck,
};
const names: Record<string, string> = {
  rule: "规则",
  ruleset: "规则集",
  router: "条件分支",
  pipeline: "子流程",
  service: "服务调用",
  start: "流程入口",
  end: "最终决策",
};
type FlowNode = Node<{ step: Step; index: number }, "policy">;
function PolicyNode({ data, selected }: NodeProps<FlowNode>) {
  const { step, index } = data;
  const Icon = icons[step.type as keyof typeof icons] || Braces;
  const routes = Array.isArray(step.routes) ? step.routes : [];
  return (
    <div className={`policy-node ${step.type} ${selected ? "selected" : ""}`}>
      {step.type !== "start" && (
        <Handle type="target" position={Position.Top} />
      )}
      <div className="node-top">
        <span className={`node-icon ${step.type}`}>
          <Icon size={18} />
        </span>
        <span>{names[step.type] || step.type}</span>
        <span className="node-number">
          {index < 0 ? "●" : String(index + 1).padStart(2, "0")}
        </span>
      </div>
      <strong>{step.name || step.id}</strong>
      <code>{step.id === "__entry" ? "entry" : step.id}</code>
      {!["start", "end", "router"].includes(step.type) && (
        <div className="node-reference">
          <ArrowRight size={12} />
          {String(step[step.type] || "尚未选择资源")}
        </div>
      )}
      {step.type === "router" ? (
        <div className="router-handles">
          {[...routes.map((_, i) => String(i)), "default"].map(
            (handle, i, handles) => (
              <Handle
                key={handle}
                id={handle}
                type="source"
                position={Position.Bottom}
                style={{ left: `${((i + 1) / (handles.length + 1)) * 100}%` }}
                title={handle === "default" ? "默认分支" : `条件 ${i + 1}`}
              />
            ),
          )}
        </div>
      ) : (
        step.type !== "end" && (
          <Handle type="source" position={Position.Bottom} />
        )
      )}
    </div>
  );
}
const nodeTypes = { policy: PolicyNode };
const edgeTypes = { condition: ConditionEdge };

function ViewControls({ onArrange }: { onArrange: () => void }) {
  const { fitView, getNodes } = useReactFlow();
  function readView(duration = 220) {
    const first = [...getNodes()]
      .sort((a, b) => a.position.y - b.position.y)
      .slice(0, 4);
    void fitView({ nodes: first, padding: 0.2, maxZoom: 1, duration });
  }
  return (
    <Panel position="top-right" className="flow-view-controls">
      <button onClick={() => readView()}>阅读视图</button>
      <button onClick={() => void fitView({ padding: 0.24, duration: 220 })}>
        查看全图
      </button>
      <button
        onClick={() => {
          onArrange();
          requestAnimationFrame(() => readView());
        }}
      >
        整理布局
      </button>
    </Panel>
  );
}

export default function PipelineCanvas({
  pipeline,
  steps,
  selected,
  onSelect,
  onConnect,
}: {
  pipeline: ObjectValue;
  steps: Step[];
  selected: string | null;
  onSelect: (id: string | null) => void;
  onConnect: (connection: Connection) => void;
}) {
  const [positions, setPositions] = useState<Record<string, XYPosition>>({});
  const { nodes, edges } = useMemo(() => {
    const edges: Edge[] = [];
    function edge(
      source: string,
      target: unknown,
      handle?: string,
      label?: string,
    ) {
      if (typeof target !== "string") return;
      edges.push({
        id: `${source}-${handle || "next"}-${target}`,
        source,
        target,
        sourceHandle: handle,
        type: "condition",
        label,
        markerEnd: {
          type: MarkerType.ArrowClosed,
          color: "var(--workflow-edge)",
        },
        style: {
          stroke: "var(--workflow-edge)",
          strokeWidth:
            selected && (selected === source || selected === target)
              ? 2.5
              : 1.5,
          strokeDasharray: handle === "default" ? "6 5" : undefined,
          opacity:
            selected && selected !== source && selected !== target ? 0.22 : 1,
        },
        labelStyle: { fill: "var(--workflow-edge-label-text)", fontSize: 12 },
        labelBgStyle: { fill: "var(--workflow-edge-label-bg)" },
        labelBgPadding: [5, 4],
      });
    }
    edge("__entry", pipeline.entry);
    steps.forEach((step) => {
      if (step.type === "router") {
        if (Array.isArray(step.routes))
          step.routes.forEach((route, index) =>
            edge(
              step.id,
              object(route).next,
              String(index),
              conditionLabel(object(route).when),
            ),
          );
        edge(step.id, step.default, "default", "默认（其余情况）");
      } else edge(step.id, step.next);
    });
    const all = [
      { id: "__entry", name: "开始处理事件", type: "start" } as Step,
      ...steps,
      { id: "end", name: "输出决策结果", type: "end" } as Step,
    ];
    const ranks = flowRanks(
      all.map((step) => step.id),
      edges,
    );
    const levels = new Map<number, string[]>();
    for (const step of all)
      levels.set(ranks[step.id], [
        ...(levels.get(ranks[step.id]) || []),
        step.id,
      ]);
    const nodes: FlowNode[] = all.map((step, i) => {
      const level = levels.get(ranks[step.id])!;
      return {
        id: step.id,
        type: "policy",
        selected: selected === step.id,
        data: { step, index: i - 1 },
        position: positions[step.id] || {
          x: (level.indexOf(step.id) - (level.length - 1) / 2) * 380,
          y: ranks[step.id] * 240,
        },
      };
    });
    const lanes = bypassLanes(nodes, edges);
    for (const edge of edges) edge.data = lanes.get(edge.id);
    return { nodes, edges };
  }, [pipeline, steps, positions, selected]);
  return (
    <ReactFlow
      fitView
      fitViewOptions={{
        nodes: [...nodes]
          .sort((a, b) => a.position.y - b.position.y)
          .slice(0, 4),
        padding: 0.2,
        maxZoom: 1,
      }}
      nodes={nodes}
      edges={edges}
      nodeTypes={nodeTypes}
      edgeTypes={edgeTypes}
      onNodeClick={(_, node) => onSelect(node.id)}
      onPaneClick={() => onSelect(null)}
      onConnect={onConnect}
      onNodesChange={(changes) => {
        for (const change of changes)
          if (change.type === "position" && change.position)
            setPositions((previous) => ({
              ...previous,
              [change.id]: change.position!,
            }));
      }}
      deleteKeyCode={null}
      minZoom={0.25}
      maxZoom={1.5}
      isValidConnection={(connection) =>
        connection.source !== connection.target &&
        connection.source !== "end" &&
        connection.target !== "__entry"
      }
    >
      <ViewControls onArrange={() => setPositions({})} />
      <Background gap={20} size={1} color="var(--workflow-canvas-dot)" />
      <Controls showInteractive={false} />
      <MiniMap
        pannable
        zoomable
        nodeColor="var(--workflow-minimap-node)"
        maskColor="var(--workflow-minimap-mask)"
      />
    </ReactFlow>
  );
}
