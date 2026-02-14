# Discovery — Prototype Retrospective

## What worked well (keep all of it)

- **Window + GPU rendering** — Own frameless window, wgpu-based terminal grid rendering
- **PTY + VTE handling** — ConPTY shell spawning, VTE escape sequence parsing
- **Tab system** — Multiple tabs, tab bar, drag/reorder
- **Font rendering** — Font shaping, glyph atlas, ligatures

## What didn't work

### Architecture (all four failure modes)

- **Too coupled** — Everything depended on everything else, hard to change one thing without breaking another
- **God objects** — App/Tab structs doing too much, unclear responsibilities
- **Wrong abstractions** — Boundaries were in the wrong places, modules didn't map to real concerns
- **Grew organically** — No clear design upfront, just kept adding code until it worked

### Threading model

- **Too much locking** — Arc<Mutex> everywhere, contention, deadlock risk
- **Wrong thread boundaries** — Work was on the wrong threads (e.g., VTE parsing on render thread)
- **Main thread bottleneck** — Too much work on the event loop thread, blocking rendering

### Performance

- General performance was acceptable for normal use
- **htop lockup** — htop specifically would lock up the entire UI. Raw throughput floods (scripted input slamming) handled fine. The issue is likely not byte volume but screen complexity: htop uses alternate screen buffer, rapid cursor repositioning, and dense per-cell SGR color sequences, causing O(rows × cols) state changes per refresh rather than linear byte processing

## Rebuild approach

- **Bottom-up, one layer at a time** — Each layer solid before building the next
- Layer order: PTY+VTE → Grid → Rendering → Tabs → Polish
