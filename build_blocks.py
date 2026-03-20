"""
Microscope Memory — Zoom-based hierarchical memory
====================================================
Concept: Data stored in uniform blocks.
Query = position + zoom level = what you see through the lens.
The block is always the same size. Only the DEPTH changes.

Depth 0: Entire identity in one block
Depth 1: Layer summaries
Depth 2: Topic clusters
Depth 3: Individual memories
Depth 4: Sentences
Depth 5: Raw tokens / embedding coordinates
"""

import json, os, hashlib, math, time, re
from pathlib import Path
from dataclasses import dataclass, field, asdict
from typing import List, Optional, Tuple

LAYERS_DIR = Path(r"D:\Claude Memory\layers")
OUTPUT_DIR = Path(r"D:\Claude Memory\microscope")
OUTPUT_DIR.mkdir(exist_ok=True)

BLOCK_SIZE = 256  # chars — fix viewport méret, minden block ennyi

# ─── Block ────────────────────────────────────────────
@dataclass
class Block:
    data: str           # fix méret, max BLOCK_SIZE chars
    depth: int          # zoom level (0=legfelső, 5=legmélyebb)
    x: float            # 3D coords
    y: float
    z: float
    source_layer: str   # melyik memória réteg
    block_id: str = ""  # hash
    children: List[str] = field(default_factory=list)  # mélyebb block id-k
    parent: str = ""    # feljebb block id

    def __post_init__(self):
        if not self.block_id:
            h = hashlib.md5(f"{self.depth}:{self.data[:64]}:{self.x:.4f}".encode()).hexdigest()[:12]
            self.block_id = f"B{self.depth}_{h}"

# ─── Coords from content hash ─────────────────────────
def content_to_coords(text: str, layer: str, index: int, total: int) -> Tuple[float, float, float]:
    """Deterministic 3D position from content + layer"""
    h = hashlib.sha256(text[:128].encode(errors='replace')).digest()
    # Base position from hash
    bx = (h[0] + h[1] * 256) / 65535.0
    by = (h[2] + h[3] * 256) / 65535.0
    bz = (h[4] + h[5] * 256) / 65535.0

    # Layer offset — each layer gets its own region
    layer_offsets = {
        'long_term': (0.0, 0.0, 0.0),
        'associative': (0.3, 0.0, 0.0),
        'emotional': (0.0, 0.3, 0.0),
        'relational': (0.3, 0.3, 0.0),
        'reflections': (0.0, 0.0, 0.3),
        'crypto_chain': (0.3, 0.0, 0.3),
        'echo_cache': (0.0, 0.3, 0.3),
        'short_term': (0.15, 0.15, 0.15),
        'rust_state': (0.15, 0.0, 0.15),
        'working': (0.0, 0.15, 0.15),
    }
    ox, oy, oz = layer_offsets.get(layer, (0.5, 0.5, 0.5))

    return (
        ox + bx * 0.25,
        oy + by * 0.25,
        oz + bz * 0.25
    )

# ─── Truncate/Pad to exact block size ──────────────────
def to_block_data(text: str) -> str:
    """Fix viewport — always exactly BLOCK_SIZE chars"""
    text = text.strip()
    if len(text) > BLOCK_SIZE:
        return text[:BLOCK_SIZE-3] + "..."
    return text  # rövidebb is OK, a lényeg: max BLOCK_SIZE

# ─── Load raw memories from layers ─────────────────────
def load_layer(name: str) -> list:
    path = LAYERS_DIR / f"{name}.json"
    if not path.exists():
        return []
    with open(path, 'r', encoding='utf-8') as f:
        data = json.load(f)
    if isinstance(data, list):
        return data
    if isinstance(data, dict):
        # associative: nodes dict, relational: entities dict, etc.
        items = []
        for key, val in data.items():
            if isinstance(val, dict):
                val['_key'] = key
                items.append(val)
            elif isinstance(val, list):
                for v in val:
                    if isinstance(v, dict):
                        v['_key'] = key
                        items.append(v)
        return items
    return []

# ─── Extract text content from memory item ─────────────
def extract_text(item: dict) -> str:
    """Get readable text from any memory format"""
    if isinstance(item, str):
        return item
    for key in ['content', 'text', 'content_summary', 'pattern', 'response', 'label', 'name']:
        if key in item and isinstance(item[key], str) and len(item[key]) > 3:
            return item[key]
    # Fallback: serialize
    return json.dumps(item, ensure_ascii=False, default=str)[:512]

# ─── Split text into sentences ──────────────────────────
def split_sentences(text: str) -> List[str]:
    """Split into sentence-level chunks"""
    parts = re.split(r'(?<=[.!?\n])\s+', text)
    return [p.strip() for p in parts if len(p.strip()) > 5]

# ─── BUILD THE MICROSCOPE ──────────────────────────────
def build_microscope():
    all_blocks: List[Block] = []
    layer_names = ['long_term', 'short_term', 'associative', 'emotional',
                   'relational', 'reflections', 'crypto_chain', 'echo_cache', 'rust_state']

    # ═══ DEPTH 0: Entire memory in one block ═══
    identity_text = "Claude Memory — 8 réteg: long_term, short_term, associative, emotional, relational, reflections, crypto_chain, echo_cache. Máté Róbert (Silent) gépe. Ora = AI partner (Rust). Hullám-rezonancia, érzelmi frekvencia, kriogenikus snapshot rendszer."
    depth0 = Block(
        data=to_block_data(identity_text),
        depth=0, x=0.25, y=0.25, z=0.25,
        source_layer='identity'
    )
    all_blocks.append(depth0)

    # ═══ DEPTH 1: Layer summaries (1 block per layer) ═══
    depth1_blocks = []
    for layer_name in layer_names:
        items = load_layer(layer_name)
        count = len(items)
        # Summary of the layer
        texts = [extract_text(it)[:60] for it in items[:5]]
        summary = f"[{layer_name}] {count} elem. " + " | ".join(texts)

        cx, cy, cz = content_to_coords(layer_name, layer_name, 0, 1)
        b = Block(
            data=to_block_data(summary),
            depth=1, x=cx, y=cy, z=cz,
            source_layer=layer_name,
            parent=depth0.block_id
        )
        depth1_blocks.append(b)
        all_blocks.append(b)

    depth0.children = [b.block_id for b in depth1_blocks]

    # ═══ DEPTH 2: Topic clusters (group items by similarity) ═══
    # Simple: every 5 items = 1 cluster block
    depth2_blocks = []
    for layer_name in layer_names:
        items = load_layer(layer_name)
        parent_b = next((b for b in depth1_blocks if b.source_layer == layer_name), None)

        cluster_size = 5
        for ci in range(0, len(items), cluster_size):
            cluster = items[ci:ci+cluster_size]
            cluster_texts = [extract_text(it)[:50] for it in cluster]
            cluster_summary = f"[{layer_name} #{ci//cluster_size}] " + " | ".join(cluster_texts)

            cx, cy, cz = content_to_coords(cluster_summary, layer_name, ci, len(items))
            b = Block(
                data=to_block_data(cluster_summary),
                depth=2, x=cx, y=cy, z=cz,
                source_layer=layer_name,
                parent=parent_b.block_id if parent_b else ""
            )
            depth2_blocks.append(b)
            all_blocks.append(b)
            if parent_b:
                parent_b.children.append(b.block_id)

    # ═══ DEPTH 3: Individual memories ═══
    depth3_blocks = []
    for layer_name in layer_names:
        items = load_layer(layer_name)
        for idx, item in enumerate(items):
            text = extract_text(item)
            cx, cy, cz = content_to_coords(text, layer_name, idx, len(items))

            # Find parent cluster
            cluster_idx = idx // 5
            parent_id = ""
            matching = [b for b in depth2_blocks if b.source_layer == layer_name]
            if cluster_idx < len(matching):
                parent_id = matching[cluster_idx].block_id
                matching[cluster_idx].children.append(f"D3_{layer_name}_{idx}")

            b = Block(
                data=to_block_data(text),
                depth=3, x=cx, y=cy, z=cz,
                source_layer=layer_name,
                parent=parent_id
            )
            depth3_blocks.append(b)
            all_blocks.append(b)

    # ═══ DEPTH 4: Sentences ═══
    depth4_blocks = []
    for d3b in depth3_blocks:
        sentences = split_sentences(d3b.data)
        for si, sent in enumerate(sentences):
            if len(sent) < 10:
                continue
            # Slight coord offset from parent
            h = hashlib.md5(sent.encode(errors='replace')).digest()
            offset = (h[0]/2550.0, h[1]/2550.0, h[2]/2550.0)
            b = Block(
                data=to_block_data(sent),
                depth=4,
                x=d3b.x + offset[0],
                y=d3b.y + offset[1],
                z=d3b.z + offset[2],
                source_layer=d3b.source_layer,
                parent=d3b.block_id
            )
            depth4_blocks.append(b)
            all_blocks.append(b)
            d3b.children.append(b.block_id)

    # ═══ DEPTH 5: Token-level (first 8 tokens per sentence) ═══
    depth5_count = 0
    for d4b in depth4_blocks:
        tokens = d4b.data.split()[:8]
        for ti, tok in enumerate(tokens):
            if len(tok) < 2:
                continue
            h = hashlib.md5(tok.encode(errors='replace')).digest()
            b = Block(
                data=to_block_data(tok),
                depth=5,
                x=d4b.x + (h[0]-128)/25500.0,
                y=d4b.y + (h[1]-128)/25500.0,
                z=d4b.z + (h[2]-128)/25500.0,
                source_layer=d4b.source_layer,
                parent=d4b.block_id
            )
            all_blocks.append(b)
            depth5_count += 1

    return all_blocks

# ─── MICROSCOPE QUERY ──────────────────────────────────
class Microscope:
    def __init__(self, blocks: List[Block]):
        self.blocks = blocks
        self.by_depth = {}
        for b in blocks:
            self.by_depth.setdefault(b.depth, []).append(b)

    def look(self, x: float, y: float, z: float, zoom: int, radius: float = 0.15) -> List[Block]:
        """
        A nagyító.
        Fókuszpont: (x, y, z)
        Zoom: melyik mélység (0-5)
        Radius: mekkora a viewport (fix!)

        Returns: blocks that fall within the viewport at that depth.
        """
        candidates = self.by_depth.get(zoom, [])
        results = []
        for b in candidates:
            dist = math.sqrt((b.x - x)**2 + (b.y - y)**2 + (b.z - z)**2)
            if dist <= radius:
                results.append((dist, b))
        results.sort(key=lambda t: t[0])
        return [b for _, b in results]

    def zoom_at(self, block_id: str) -> Optional[Block]:
        """Find a specific block by ID"""
        for b in self.blocks:
            if b.block_id == block_id:
                return b
        return None

    def drill_down(self, block: Block) -> List[Block]:
        """Zoom in — get children"""
        child_ids = set(block.children)
        return [b for b in self.blocks if b.block_id in child_ids]

    def zoom_out(self, block: Block) -> Optional[Block]:
        """Zoom out — get parent"""
        if block.parent:
            return self.zoom_at(block.parent)
        return None

    def stats(self):
        print(f"\n{'='*50}")
        print(f"  MICROSCOPE MEMORY")
        print(f"{'='*50}")
        total = len(self.blocks)
        print(f"  Total blocks: {total}")
        print(f"  Block size:   max {BLOCK_SIZE} chars (fix viewport)")
        print(f"  Depths:")
        for d in sorted(self.by_depth.keys()):
            print(f"    Depth {d}: {len(self.by_depth[d]):>6} blocks")
        print(f"{'='*50}\n")


# ─── VECTOR INDEX (numpy L2) ───────────────────────────
class VectorMicroscope:
    """
    Vektor alapu mikroszkop.
    Minden block = 4D vektor: [x, y, z, depth_normalized]
    Query = [x, y, z, zoom_normalized] + L2 distance
    EGY lekerdezes, az adat BENNE VAN a vektorban.
    """
    def __init__(self, blocks: List[Block]):
        import numpy as np
        self.blocks = blocks
        self.np = np
        self.vectors = np.zeros((len(blocks), 4), dtype=np.float32)
        for i, b in enumerate(blocks):
            self.vectors[i] = [b.x, b.y, b.z, b.depth / 5.0]
        self.depths = np.array([b.depth for b in blocks], dtype=np.int32)

    def look(self, x: float, y: float, z: float, zoom: int,
             k: int = 10, zoom_weight: float = 2.0) -> List[Tuple[float, Block]]:
        """4D vector search, zoom as weighted dimension"""
        np = self.np
        q = np.array([x, y, z, zoom / 5.0], dtype=np.float32)
        weights = np.array([1.0, 1.0, 1.0, zoom_weight], dtype=np.float32)
        diff = (self.vectors - q) * weights
        dists = np.sum(diff * diff, axis=1)
        top = min(k, len(dists))
        idx = np.argpartition(dists, top)[:top]
        idx = idx[np.argsort(dists[idx])]
        return [(float(dists[i]), self.blocks[i]) for i in idx]

    def look_depth(self, x: float, y: float, z: float, zoom: int,
                   k: int = 10) -> List[Tuple[float, Block]]:
        """Exact depth match + spatial L2"""
        np = self.np
        mask = self.depths == zoom
        if not mask.any():
            return []
        indices = np.where(mask)[0]
        vecs = self.vectors[indices, :3]
        q = np.array([x, y, z], dtype=np.float32)
        dists = np.sum((vecs - q) ** 2, axis=1)
        top = min(k, len(dists))
        local_idx = np.argpartition(dists, top)[:top]
        local_idx = local_idx[np.argsort(dists[local_idx])]
        return [(float(dists[li]), self.blocks[indices[li]]) for li in local_idx]


LAYER_COLORS = {
    'identity': 'white', 'long_term': 'blue', 'short_term': 'cyan',
    'associative': 'green', 'emotional': 'red', 'relational': 'yellow',
    'reflections': 'magenta', 'crypto_chain': 'orange',
    'echo_cache': 'lime', 'rust_state': 'purple',
}


# ─── MAIN: BUILD + TEST ────────────────────────────────
if __name__ == "__main__":
    print("Building microscope memory from Claude's 8-layer memory...")
    t0 = time.time()
    blocks = build_microscope()
    elapsed = time.time() - t0
    print(f"Built {len(blocks)} blocks in {elapsed:.2f}s")

    scope = Microscope(blocks)
    scope.stats()

    print("Building vector index...")
    t0 = time.time()
    vscope = VectorMicroscope(blocks)
    print(f"Vector index: {vscope.vectors.shape} in {time.time()-t0:.3f}s\n")

    # --- TEST 1: Vector L2 per zoom ---
    print("TEST 1: Same point (0.25, 0.25, 0.25), vector L2 per zoom")
    print("-" * 60)
    for zoom in range(6):
        results = vscope.look_depth(0.25, 0.25, 0.25, zoom, k=5)
        print(f"\n  ZOOM {zoom} -> {len(results)} results:")
        for dist, b in results[:3]:
            preview = b.data[:65].replace('\n', ' ')
            color = LAYER_COLORS.get(b.source_layer, '?')
            print(f"    L2={dist:.4f} [{b.source_layer}/{color}] {preview}")

    # --- TEST 2: Drill down ---
    print(f"\n\nTEST 2: Drill down from top")
    print("-" * 60)
    top = blocks[0]
    print(f"  Depth {top.depth}: {top.data[:90]}")
    children = scope.drill_down(top)
    print(f"  -> {len(children)} children at depth 1:")
    for c in children[:4]:
        print(f"    [{c.source_layer}] {c.data[:65]}")

    # --- TEST 3: Keyword search per zoom ---
    print(f"\n\nTEST 3: Find 'Ora' at different depths")
    print("-" * 60)
    for zoom in range(5):
        hits = [b for b in scope.by_depth.get(zoom, []) if 'Ora' in b.data or 'ora' in b.data]
        print(f"  ZOOM {zoom}: {len(hits)} blocks contain 'Ora'")
        if hits:
            print(f"    -> {hits[0].data[:65]}")

    # --- TEST 4: Vector query speed (1000x) ---
    print(f"\n\nTEST 4: Vector query speed (numpy L2, 1000 queries)")
    print("-" * 60)
    import random
    random.seed(42)
    for zoom in range(6):
        times = []
        for _ in range(1000):
            rx, ry, rz = random.random()*0.5, random.random()*0.5, random.random()*0.5
            t0 = time.time()
            vscope.look_depth(rx, ry, rz, zoom, k=5)
            times.append(time.time() - t0)
        avg_us = sum(times)/len(times) * 1_000_000
        n = len(scope.by_depth.get(zoom, []))
        print(f"  ZOOM {zoom}: avg {avg_us:.1f} us/query ({n} blocks)")

    # --- TEST 5: 4D soft zoom ---
    print(f"\n\nTEST 5: 4D vector search (zoom as dimension, weight=2.0)")
    print("-" * 60)
    for zoom in range(6):
        results = vscope.look(0.15, 0.15, 0.15, zoom, k=5, zoom_weight=2.0)
        depths_found = [b.depth for _, b in results]
        print(f"  Query zoom={zoom} -> depths: {depths_found}")
        if results:
            _, b = results[0]
            color = LAYER_COLORS.get(b.source_layer, '?')
            print(f"    Best: [{b.source_layer}/{color}] d={b.depth} {b.data[:55]}")

    # --- Save ---
    out_path = OUTPUT_DIR / "microscope_blocks.json"
    export = [asdict(b) for b in blocks]
    with open(out_path, 'w', encoding='utf-8') as f:
        json.dump(export, f, ensure_ascii=False, indent=1)
    print(f"\nSaved {len(blocks)} blocks to {out_path}")
    print(f"File size: {out_path.stat().st_size / 1024:.1f} KB")
