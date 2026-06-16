#!/usr/bin/env python3
"""
Microscope Memory Python Demo
Demonstrates using the Rust-powered memory system from Python
"""

# After building with: maturin develop --features python
# or: pip install microscope-memory

from microscope_memory import PyMicroscope, PyBlock
import numpy as np
import time

def demo():
    """Main demo function"""
    print("🔬 Microscope Memory - Python Demo")
    print("=" * 50)

    # Initialize
    microscope = PyMicroscope()
    print(f"Initialized Microscope Memory")

    # Add some test blocks
    print("\n📦 Adding test blocks...")
    test_data = [
        ("AI language model", 0.1, 0.1, 0.1, 3, 0),
        ("Neural network training", 0.2, 0.1, 0.1, 3, 0),
        ("Deep learning framework", 0.3, 0.2, 0.1, 3, 1),
        ("Memory management system", 0.4, 0.3, 0.2, 3, 1),
        ("Hierarchical data structure", 0.5, 0.4, 0.3, 3, 2),
        ("Vector database", 0.6, 0.5, 0.4, 3, 2),
        ("Embedding space", 0.7, 0.6, 0.5, 3, 3),
        ("Semantic search", 0.8, 0.7, 0.6, 3, 3),
    ]

    for text, x, y, z, depth, layer_id in test_data:
        microscope.add_block(text, x, y, z, depth, layer_id)

    print(f"Added {microscope.block_count()} blocks")

    # Test semantic search
    print("\n🔍 Testing semantic search...")
    query = "neural network"
    start = time.time()
    results = microscope.semantic_search(query, k=3)
    elapsed = (time.time() - start) * 1000

    print(f"Query: '{query}'")
    print(f"Found {len(results)} results in {elapsed:.2f}ms:")
    for i, block in enumerate(results, 1):
        print(f"  {i}. [{block.layer_id}] {block.text[:50]} (sim={block.similarity:.3f})")

    # Test spatial search
    print("\n📍 Testing spatial search...")
    x, y, z = 0.3, 0.2, 0.1
    radius = 0.3
    start = time.time()
    results = microscope.spatial_search(x, y, z, radius, depth=3)
    elapsed = (time.time() - start) * 1000

    print(f"Center: ({x}, {y}, {z}), Radius: {radius}")
    print(f"Found {len(results)} blocks in {elapsed:.2f}ms:")
    for i, block in enumerate(results[:3], 1):
        print(f"  {i}. {block.text[:50]} (dist={1-block.similarity:.3f})")

    # Test hybrid search
    print("\n🎯 Testing hybrid search...")
    query = "memory"
    x, y, z = 0.5, 0.4, 0.3
    start = time.time()
    results = microscope.hybrid_search(
        query, x, y, z,
        semantic_weight=0.6,
        spatial_weight=0.4,
        k=3
    )
    elapsed = (time.time() - start) * 1000

    print(f"Query: '{query}' at ({x}, {y}, {z})")
    print(f"Found {len(results)} results in {elapsed:.2f}ms:")
    for i, block in enumerate(results, 1):
        print(f"  {i}. {block.text[:50]} (score={block.similarity:.3f})")

    # Export and reimport
    print("\n💾 Testing export/import...")
    exported = microscope.export_blocks()
    print(f"Exported {len(exported)} blocks")

    microscope.clear()
    print(f"Cleared. Block count: {microscope.block_count()}")

    # Reimport
    from types import SimpleNamespace
    py_list = [(text, x, y, z, depth, layer) for text, x, y, z, depth, layer in exported]

    for item in py_list:
        microscope.add_block(*item)

    print(f"Reimported. Block count: {microscope.block_count()}")

    # Stats
    print("\n📊 Statistics:")
    print(microscope.stats())

    print("\n✅ Demo complete!")


def benchmark():
    """Performance benchmark"""
    print("\n⚡ Performance Benchmark")
    print("=" * 50)

    microscope = PyMicroscope()

    # Add many blocks
    print("Adding 10,000 blocks...")
    for i in range(10000):
        text = f"Block {i}: " + "x" * 100
        x = np.random.random()
        y = np.random.random()
        z = np.random.random()
        depth = np.random.randint(0, 6)
        layer = np.random.randint(0, 5)
        microscope.add_block(text, x, y, z, depth, layer)

    print(f"Total blocks: {microscope.block_count()}")

    # Benchmark searches
    queries = ["test", "block", "memory", "system", "data"]
    total_time = 0

    print("\nRunning 100 semantic searches...")
    for _ in range(20):
        for query in queries:
            start = time.time()
            results = microscope.semantic_search(query, k=10)
            total_time += time.time() - start

    avg_time = (total_time / 100) * 1000
    print(f"Average semantic search: {avg_time:.2f}ms")

    # Spatial search benchmark
    total_time = 0
    print("\nRunning 100 spatial searches...")
    for _ in range(100):
        x, y, z = np.random.random(3)
        start = time.time()
        results = microscope.spatial_search(x, y, z, 0.1)
        total_time += time.time() - start

    avg_time = (total_time / 100) * 1000
    print(f"Average spatial search: {avg_time:.2f}ms")

    print("\n✅ Benchmark complete!")


if __name__ == "__main__":
    demo()
    benchmark()