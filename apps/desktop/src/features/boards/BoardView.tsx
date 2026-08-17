import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ReactFlow,
  Background,
  Controls,
  MiniMap,
  Panel,
  useNodesState,
  useEdgesState,
  useViewport,
  useReactFlow,
  addEdge,
  type OnNodesChange,
  type OnEdgesChange,
  type OnConnect,
  type OnMoveEnd,
  type Edge,
  type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { readText } from "@tauri-apps/plugin-clipboard-manager";

import {
  getBoardSnapshot,
  moveNodes,
  saveViewport,
  createEdge,
  deleteEdge,
  createBoard,
  placeBoardLink,
  placeItemOnBoard,
  bringNodeToFront,
  setNodeLocked,
  removeNode,
  groupNodes,
  ungroupNodes,
} from "./api";
import { captureText } from "../inbox/api";
import {
  hasInboxItemDragPayload,
  readInboxItemDragPayload,
} from "../inbox/dragPayload";
import type { BoardSummary } from "./api";
import { toFlowNodes, toFlowEdges } from "../../adapters/canvas/CanvasMapper";
import type { CanvasNode } from "../../adapters/canvas/CanvasMapper";
import { computeKeyboardMoves } from "../../adapters/canvas/canvasKeyboard";
import { ItemCardNode } from "./ItemCardNode";
import { BoardLinkNode } from "./BoardLinkNode";
import { GroupNode } from "./GroupNode";
import { useUndoStore, createMoveCommand, createRemoveNodeCommand, createGroupCommand } from "../../stores/undoStore";

const nodeTypes = {
  itemCard: ItemCardNode,
  boardLink: BoardLinkNode,
  group: GroupNode,
};

interface BoardViewProps {
  board: BoardSummary;
  /** BRD-004: called when the user double-clicks a board_link node. */
  onEnterBoard?: (targetBoardId: string) => void;
}

function ViewportSaver({ boardId }: { boardId: string }) {
  const { x, y, zoom } = useViewport();
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => {
      void saveViewport(boardId, x, y, zoom);
    }, 1000);
    return () => {
      if (debounceRef.current) clearTimeout(debounceRef.current);
    };
  }, [boardId, x, y, zoom]);

  return null;
}

/** BRD-002: paste clipboard content at the current mouse position.
 * Must render inside <ReactFlow> to access useReactFlow coordinate conversion.
 */
function CanvasPasteHandler({
  boardId,
  onPlaced,
}: {
  boardId: string;
  onPlaced: () => void;
}) {
  const { screenToFlowPosition } = useReactFlow();
  const mousePos = useRef({ x: window.innerWidth / 2, y: window.innerHeight / 2 });

  useEffect(() => {
    const trackMouse = (e: MouseEvent) => {
      mousePos.current = { x: e.clientX, y: e.clientY };
    };
    window.addEventListener("mousemove", trackMouse);

    const handler = (e: KeyboardEvent) => {
      if (!((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "v")) return;
      // Don't hijack paste into inputs
      const tag = (e.target as HTMLElement | null)?.tagName;
      if (tag === "INPUT" || tag === "TEXTAREA") return;
      e.preventDefault();

      void (async () => {
        try {
          const text = await readText();
          if (!text || !text.trim()) return;
          const result = await captureText({
            text: text.trim(),
            captureMethod: "canvas_paste",
          });
          const pos = screenToFlowPosition({
            x: mousePos.current.x,
            y: mousePos.current.y,
          });
          await placeItemOnBoard(boardId, result.itemId, pos.x, pos.y);
          onPlaced();
        } catch {
          // Paste failed silently — user can retry
        }
      })();
    };
    window.addEventListener("keydown", handler);
    return () => {
      window.removeEventListener("mousemove", trackMouse);
      window.removeEventListener("keydown", handler);
    };
  }, [boardId, screenToFlowPosition, onPlaced]);

  return null;
}

/** INB-003: drop an Inbox item onto the canvas at the pointer position.
 * Rendered inside <ReactFlow> so useReactFlow coordinate conversion works. */
function InboxItemDropHandler({
  boardId,
  onPlaced,
}: {
  boardId: string;
  onPlaced: () => void;
}) {
  const { screenToFlowPosition } = useReactFlow();

  useEffect(() => {
    const onDragOver = (e: DragEvent) => {
      if (!hasInboxItemDragPayload(e.dataTransfer)) return;
      e.preventDefault(); // allow the drop
      if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    };
    const onDrop = (e: DragEvent) => {
      const itemId = readInboxItemDragPayload(e.dataTransfer);
      if (!itemId) return; // unrelated drag (files, text, …)
      e.preventDefault();
      const pos = screenToFlowPosition({ x: e.clientX, y: e.clientY });
      void placeItemOnBoard(boardId, itemId, pos.x, pos.y).then(() => onPlaced());
    };
    window.addEventListener("dragover", onDragOver);
    window.addEventListener("drop", onDrop);
    return () => {
      window.removeEventListener("dragover", onDragOver);
      window.removeEventListener("drop", onDrop);
    };
  }, [boardId, screenToFlowPosition, onPlaced]);

  return null;
}

/** Workstream G: fit-view hotkey (F). Must render inside <ReactFlow> to
 * reach the useReactFlow context; honors prefers-reduced-motion. */
function FitViewHotkey() {
  const { fitView } = useReactFlow();

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || e.key.toLowerCase() !== "f") return;
      const target = e.target as HTMLElement | null;
      if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
      e.preventDefault();
      const reduce = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
      void fitView({ duration: reduce ? 0 : 200 });
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [fitView]);

  return null;
}

/** BRD-004: creates a sub-board and places its link node at the viewport center. */
function SubBoardCreator({
  boardId,
  onCreated,
}: {
  boardId: string;
  onCreated: () => void;
}) {
  const { x, y, zoom } = useViewport();
  const [busy, setBusy] = useState(false);

  const handleClick = useCallback(async () => {
    if (busy) return;
    setBusy(true);
    try {
      const newBoard = await createBoard("子看板");
      // Place the link near the current viewport center (flow coordinates)
      const fx = (window.innerWidth / 2 - x) / zoom;
      const fy = (window.innerHeight / 2 - y) / zoom;
      await placeBoardLink(boardId, newBoard.id, fx, fy);
      onCreated();
    } catch {
      // Ignore — user can retry
    } finally {
      setBusy(false);
    }
  }, [busy, boardId, x, y, zoom, onCreated]);

  return (
    <Panel position="top-right">
      <div className="canvas-toolbar">
        <button className="btn-sub-board" onClick={() => void handleClick()} disabled={busy}>
          {busy ? "建立中…" : "+ 子看板"}
        </button>
      </div>
    </Panel>
  );
}

export function BoardView({ board, onEnterBoard }: BoardViewProps) {
  const [nodes, setNodes, onNodesChange] = useNodesState<CanvasNode>([]);
  const [edges, setEdges, onEdgesChange] = useEdgesState<Edge>([]);
  const [loading, setLoading] = useState(true);
  const [reloadKey, setReloadKey] = useState(0);
  const originalPositions = useRef(new Map<string, { x: number; y: number }>());
  const { setBoard, push, undo, redo } = useUndoStore();

  // Workstream G: keyboard movement state (debounced batch persistence)
  const nodesRef = useRef<CanvasNode[]>([]);
  nodesRef.current = nodes;
  const pendingMoves = useRef(new Map<string, { nodeId: string; x: number; y: number }>());
  const keyboardOrigins = useRef(new Map<string, { x: number; y: number }>());
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    return () => {
      if (persistTimer.current) clearTimeout(persistTimer.current);
    };
  }, []);

  useEffect(() => {
    setBoard(board.id);
  }, [board.id, setBoard]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void getBoardSnapshot(board.id).then((snapshot) => {
      if (cancelled) return;
      const flowNodes = toFlowNodes(snapshot.nodes);
      const flowEdges = toFlowEdges(snapshot.edges);
      setNodes(flowNodes);
      setEdges(flowEdges);
      originalPositions.current = new Map(
        flowNodes.map((n) => [n.id, { ...n.position }]),
      );
      setLoading(false);
    });
    return () => { cancelled = true; };
  }, [board.id, setNodes, setEdges, reloadKey]);

  // BRD-004: double-click a board_link node to navigate into the sub-board
  const handleNodeDoubleClick = useCallback(
    (_event: React.MouseEvent, node: Node) => {
      if (node.type === "boardLink") {
        const target = (node.data as { targetBoardId?: string | null }).targetBoardId;
        if (target && onEnterBoard) {
          onEnterBoard(target);
        }
      }
    },
    [onEnterBoard],
  );

  const handleSubBoardCreated = useCallback(() => {
    setReloadKey((k) => k + 1);
  }, []);

  // BRD-010: children of collapsed groups stay in the DB but are hidden.
  const displayNodes = useMemo(() => {
    const collapsedGroups = new Set(
      nodes
        .filter((n) => n.type === "group" && (n.data as { collapsed?: boolean }).collapsed)
        .map((n) => n.id),
    );
    if (collapsedGroups.size === 0) return nodes;
    return nodes.filter((n) => !n.parentId || !collapsedGroups.has(n.parentId));
  }, [nodes]);

  // BRD-010: raising a node on drag start keeps z-order intentional.
  const handleNodeDragStart = useCallback(
    (_event: unknown, node: CanvasNode) => {
      void bringNodeToFront(board.id, node.id);
      setNodes((ns) => {
        const maxZ = ns.reduce(
          (m, n) => Math.max(m, Number(n.style?.zIndex ?? 0)),
          0,
        );
        return ns.map((n) =>
          n.id === node.id ? { ...n, style: { ...n.style, zIndex: maxZ + 1 } } : n,
        );
      });
    },
    [board.id, setNodes],
  );

  const refreshCanvas = useCallback(() => {
    setReloadKey((k) => k + 1);
  }, []);

  const handleUndo = useCallback(async () => {
    await undo();
    refreshCanvas();
  }, [undo, refreshCanvas]);

  const handleRedo = useCallback(async () => {
    await redo();
    refreshCanvas();
  }, [redo, refreshCanvas]);

  const handleRemoveSelected = useCallback(() => {
    const selected = nodesRef.current.filter((n) => n.selected);
    if (selected.length === 0) return;
    const ids = new Set(selected.map((n) => n.id));
    for (const n of selected) {
      void removeNode(board.id, n.id);
      const itemId = n.data.itemId;
      if (itemId) {
        push(createRemoveNodeCommand(board.id, n.id, itemId, n.position.x, n.position.y));
      }
    }
    setNodes((ns) => ns.filter((n) => !ids.has(n.id)));
    setEdges((es) => es.filter((edge) => !ids.has(edge.source) && !ids.has(edge.target)));
  }, [board.id, push, setNodes, setEdges]);

  const handleGroupSelected = useCallback(async () => {
    const selected = nodesRef.current.filter((n) => n.selected && n.type !== "group");
    if (selected.length < 2) return;
    const nodeIds = selected.map((n) => n.id);
    try {
      const groupId = await groupNodes(board.id, nodeIds);
      push(createGroupCommand(board.id, nodeIds, groupId));
      refreshCanvas();
    } catch {
      /* ignore */
    }
  }, [board.id, push, refreshCanvas]);

  const handleUngroupSelected = useCallback(async () => {
    const groups = nodesRef.current.filter((n) => n.selected && n.type === "group");
    if (groups.length === 0) return;
    try {
      for (const g of groups) {
        await ungroupNodes(board.id, g.id);
      }
      refreshCanvas();
    } catch {
      /* ignore */
    }
  }, [board.id, refreshCanvas]);

  // Keyboard shortcuts: undo/redo + Workstream G node movement & lock
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const target = e.target as HTMLElement | null;
      if (
        target &&
        (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)
      ) {
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key === "z") {
        e.preventDefault();
        if (e.shiftKey) {
          void handleRedo();
        } else {
          void handleUndo();
        }
        return;
      }

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "g") {
        e.preventDefault();
        if (e.shiftKey) {
          void handleUngroupSelected();
        } else {
          void handleGroupSelected();
        }
        return;
      }

      if (!e.ctrlKey && !e.metaKey && !e.altKey && (e.key === "Delete" || e.key === "Backspace")) {
        e.preventDefault();
        handleRemoveSelected();
        return;
      }

      // Lock/unlock every selected node (L)
      if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.toLowerCase() === "l") {
        const selected = nodesRef.current.filter((n) => n.selected);
        if (selected.length === 0) return;
        e.preventDefault();
        const next = !(selected[0]?.data.locked === true);
        for (const n of selected) {
          void setNodeLocked(board.id, n.id, next);
        }
        setNodes((ns) =>
          ns.map((n) =>
            n.selected
              ? { ...n, draggable: !next, data: { ...n.data, locked: next } }
              : n,
          ),
        );
        return;
      }

      // Arrow-key movement (Shift = fine 1px step); locked nodes stay put
      if (e.ctrlKey || e.metaKey || e.altKey) return;
      const movable = nodesRef.current
        .filter((n) => n.selected)
        .map((n) => ({
          id: n.id,
          x: n.position.x,
          y: n.position.y,
          locked: n.data.locked === true,
        }));
      const moves = computeKeyboardMoves(movable, e.key, e.shiftKey);
      if (!moves) return;
      e.preventDefault();
      setNodes((ns) =>
        ns.map((n) => {
          const m = moves.find((mv) => mv.nodeId === n.id);
          return m ? { ...n, position: { x: m.x, y: m.y } } : n;
        }),
      );
      for (const m of moves) {
        if (!keyboardOrigins.current.has(m.nodeId)) {
          const orig = originalPositions.current.get(m.nodeId);
          keyboardOrigins.current.set(m.nodeId, orig ?? { x: m.x, y: m.y });
        }
        pendingMoves.current.set(m.nodeId, m);
      }
      // Debounced batch persist — no DB write per keystroke frame
      if (persistTimer.current) clearTimeout(persistTimer.current);
      persistTimer.current = setTimeout(() => {
        const batch = [...pendingMoves.current.values()];
        pendingMoves.current.clear();
        const prev = [...keyboardOrigins.current.entries()].map(([nodeId, pos]) => ({
          nodeId,
          x: pos.x,
          y: pos.y,
        }));
        keyboardOrigins.current.clear();
        if (batch.length > 0) {
          void moveNodes(board.id, batch);
          push(createMoveCommand(board.id, batch, prev));
          for (const m of batch) {
            originalPositions.current.set(m.nodeId, { x: m.x, y: m.y });
          }
        }
      }, 400);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [
    board.id,
    setNodes,
    handleUndo,
    handleRedo,
    handleRemoveSelected,
    handleGroupSelected,
    handleUngroupSelected,
    push,
  ]);

  const handleNodesChange: OnNodesChange<CanvasNode> = useCallback(
    (changes) => {
      onNodesChange(changes);
    },
    [onNodesChange],
  );

  const handleEdgesChange: OnEdgesChange<Edge> = useCallback(
    (changes) => {
      onEdgesChange(changes);
    },
    [onEdgesChange],
  );

  // BRD-005: create edge when user connects two nodes
  const handleConnect: OnConnect = useCallback(
    (connection) => {
      if (!connection.source || !connection.target) return;
      void createEdge(board.id, connection.source, connection.target).then((edgeId) => {
        setEdges((eds) =>
          addEdge(
            {
              id: edgeId,
              source: connection.source!,
              target: connection.target!,
              type: "default",
            },
            eds,
          ),
        );
      });
    },
    [board.id, setEdges],
  );

  // BRD-005: persist edge deletion
  const handleEdgesDelete = useCallback(
    (edgesToDelete: Edge[]) => {
      for (const edge of edgesToDelete) {
        void deleteEdge(board.id, edge.id);
      }
    },
    [board.id],
  );

  const handleMoveEnd: OnMoveEnd = useCallback(
    (_event, viewport) => {
      void saveViewport(board.id, viewport.x, viewport.y, viewport.zoom);
    },
    [board.id],
  );

  const handleNodeDragStop = useCallback(
    (_event: unknown, node: CanvasNode) => {
      const orig = originalPositions.current.get(node.id);
      if (orig && (orig.x !== node.position.x || orig.y !== node.position.y)) {
        const moves = [{ nodeId: node.id, x: node.position.x, y: node.position.y }];
        const prevPositions = [{ nodeId: node.id, x: orig.x, y: orig.y }];
        void moveNodes(board.id, moves);
        push(createMoveCommand(board.id, moves, prevPositions));
        originalPositions.current.set(node.id, { ...node.position });
      }
    },
    [board.id, push],
  );

  if (loading) {
    return <div className="board-loading">載入看板中…</div>;
  }

  return (
    <div className="board-canvas">
      <ReactFlow
        nodes={displayNodes}
        edges={edges}
        nodeTypes={nodeTypes}
        onNodesChange={handleNodesChange}
        onEdgesChange={handleEdgesChange}
        onConnect={handleConnect}
        onEdgesDelete={handleEdgesDelete}
        onNodeDragStart={handleNodeDragStart}
        onNodeDragStop={handleNodeDragStop}
        onNodeDoubleClick={handleNodeDoubleClick}
        onMoveEnd={handleMoveEnd}
        defaultViewport={{
          x: board.viewportX,
          y: board.viewportY,
          zoom: board.viewportZoom,
        }}
        fitView={nodes.length === 0}
        minZoom={0.1}
        maxZoom={4}
        deleteKeyCode={null}
      >
        <Background gap={20} size={1} />
        <Controls />
        <MiniMap
          nodeStrokeWidth={3}
          nodeColor="#6c9fff"
          maskColor="rgba(0,0,0,0.7)"
        />
        <ViewportSaver boardId={board.id} />
        <FitViewHotkey />
        <SubBoardCreator boardId={board.id} onCreated={handleSubBoardCreated} />
        <Panel position="top-left">
          <div className="canvas-toolbar">
            <button className="btn-sub-board" onClick={() => void handleGroupSelected()}>
              群組
            </button>
            <button className="btn-sub-board" onClick={() => void handleUngroupSelected()}>
              解散
            </button>
            <button className="btn-sub-board" onClick={handleRemoveSelected}>
              移除
            </button>
          </div>
        </Panel>
        <CanvasPasteHandler boardId={board.id} onPlaced={handleSubBoardCreated} />
        <InboxItemDropHandler boardId={board.id} onPlaced={handleSubBoardCreated} />
      </ReactFlow>
    </div>
  );
}
