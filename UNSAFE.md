# Unsafe inventory

Every `unsafe` block in the crate, what it is for, and what a reviewer has to
check before trusting it. This records what is in the tree; it does not certify
that the code is sound. Nothing here has been run under Miri or a sanitizer, and
the Windows-only paths have not been exercised on a live system, so the honest
reading of this file is "here is the surface, here is what is already
justified, here is what is not".

## How these numbers were derived

Blocks are brace-matched `unsafe {` occurrences in `src/*.rs`. A block counts as
documented when a comment containing `SAFETY` or `Safety` appears within the four
lines above it. Both rules are mechanical, so the counts can be re-derived:

    57 blocks, 11 files
    15 with an adjacent safety note
    42 without one

Line numbers move; the categories below do not.

## By category

| Category | Blocks | Files | What the invariant has to hold |
|---|---:|---|---|
| mmap read | 26 | reader, embedding_index, text_index, wasm | the offset is inside the mapping, and the file has not been truncated under the reader |
| named-pipe IPC | 12 | ai_adapter | the handle is open and owned by exactly one owner; buffer length matches what the pipe reported |
| seqlock / mmap snapshot | 11 | consciousness_seqlock, consciousness_stream | the sequence number brackets the read, and the snapshot is `repr(C)` with no padding holes |
| repr(C) transmute | 7 | ai_adapter, viz | the source buffer is exactly the struct size, and the struct has no padding |
| SIMD intrinsics | 2 | reader, embeddings | the pointer is readable for the width loaded, and is unaligned-safe |
| console codepage | 1 | mcp | Windows only; sets UTF-8 for the console before any output |
| header byte view | 1 | build | the header struct is exactly `HEADER_SIZE` bytes, so reinterpreting it as bytes is sound |

The two that carry real weight are the first two: mmap reads are on the recall
hot path, and the IPC transmutes are the only place a foreign process hands this
crate bytes that were not validated by the type system.

## What is already justified

- `reader.rs:244` — `header_unchecked` carries a `# Safety` section naming the
  precondition (`i < block_count`) and states that violating it is undefined
  behaviour. Its caller `header()` checks with an `assert!` first, so the public
  path is guarded and the raw one is opt-in.
- `consciousness_seqlock.rs:201, 348, 565` — `# Safety` sections on the three
  public unsafe entry points.
- `reader.rs:147, 179, 190`, `text_index.rs:148`, `embedding_index.rs:126, 173,
  197, 211, 523`, `embeddings.rs:607`, `consciousness_seqlock.rs:146, 264, 296,
  321, 368, 381, 453, 492`, `consciousness_stream.rs:367` — `// SAFETY:` notes at
  the point of use.

## The review queue: 42 blocks without a note

    mmap read            19
    named-pipe IPC       12
    seqlock / snapshot    6
    header byte view      1
    seqlock publish       1
    SIMD                  1
    console codepage      1
    repr(C) transmute     1

Two of these are worth pulling forward rather than leaving in a queue.

**`ai_adapter.rs` transmutes bytes that arrived from another process. Checked, and
it holds.** At 412 the buffer is `[u8; 256]`, filled by `read_exact`, which loops
until the buffer is full and returns `Err("Pipe closed")` when the pipe reports
zero bytes -- so by the time 419 runs, every byte is initialised. `AICommand` is
`#[repr(C)]` with `u64 + f32 + u8 + u8 + [u8; 242]`, which is 256 bytes with no
interior padding, so `transmute::<[u8; 256], AICommand>` is sound. The pattern
at 462 and the responses at 425, 466, 438 and 478 are the same shape.

That is a statement about memory safety, not about trust. A peer on the named
pipe still chooses `op_code`, `layer` and `block_id`, and none of that is
checked here; that belongs to whatever `process_command` validates. Worth a look,
but it is input validation, not undefined behaviour.

**`consciousness_seqlock.rs:268, 322, 370`** are a seqlock: the read is a plain
dereference plus an acquire fence. The correctness argument depends on the
writer not reordering its stores relative to the sequence bump, which is an
argument about the compiler and the memory model rather than about this code,
and it is the kind of thing that wants a test under a tool that understands the
model.

## What would settle the rest

- A negative test for the IPC path: a pipe that returns fewer bytes than the
  struct, which should be refused rather than transmuted.
- A truncation test for the mmap readers: `data.bin` and `microscope.bin`
  shortened after the reader opened, which the audit also asks for.
- `cargo +nightly miri test` on the seqlock and transmute modules, where the
  construction allows it. Windows FFI blocks are out of reach for Miri.
