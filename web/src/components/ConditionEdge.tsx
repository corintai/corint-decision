import {
  BaseEdge,
  EdgeLabelRenderer,
  getSmoothStepPath,
  type EdgeProps,
} from "@xyflow/react";
import { bypassPath } from "./flowLayout";

export default function ConditionEdge(props: EdgeProps) {
  const [directPath, labelX, labelY] = getSmoothStepPath({
    sourceX: props.sourceX,
    sourceY: props.sourceY,
    sourcePosition: props.sourcePosition,
    targetX: props.targetX,
    targetY: props.targetY,
    targetPosition: props.targetPosition,
    borderRadius: 5,
  });
  const label = String(props.label ?? "");
  const fallback = props.sourceHandleId === "default";
  const lane = typeof props.data?.laneX === "number" ? props.data.laneX : null;
  const path =
    lane === null
      ? directPath
      : bypassPath(
          props.sourceX,
          props.sourceY,
          props.targetX,
          props.targetY,
          lane,
          Number(props.data?.arrivalOffset ?? 24),
        );
  const x = lane === null ? labelX : (props.sourceX + lane) / 2;
  const y = lane === null ? labelY : props.sourceY + 28;
  return (
    <>
      <BaseEdge
        id={props.id}
        path={path}
        markerEnd={props.markerEnd}
        style={props.style}
        interactionWidth={props.interactionWidth}
      />
      {label && (
        <EdgeLabelRenderer>
          <div
            className={`condition-edge-label nodrag nopan ${fallback ? "fallback" : ""}`}
            title={label}
            style={{
              transform: `translate(${lane !== null ? "-50%" : fallback ? "0%" : "-100%"}, -50%) translate(${x}px, ${y}px)`,
              opacity: props.style?.opacity,
            }}
          >
            {label}
          </div>
        </EdgeLabelRenderer>
      )}
    </>
  );
}
