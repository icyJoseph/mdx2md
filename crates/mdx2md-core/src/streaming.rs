//! Streaming HTML → Markdown converter for React SSR with Suspense.
//!
//! React's out-of-order streaming protocol delivers HTML in chunks:
//!
//! 1. **Initial shell** — contains `<!--$?-->` markers for Suspense boundaries
//!    followed by fallback content and closed with `<!--/$-->`.
//! 2. **Resolution chunks** — `<div hidden id="S:X">resolved content</div>`
//!    containing the actual content for a boundary.
//! 3. **Swap scripts** — `$RC("B:X","S:X")` that instruct the client to replace
//!    the fallback content with the resolved content.
//!
//! A deeply nested boundary can resolve before its parent. The [`StreamConverter`]
//! buffers at boundary level and only emits Markdown for a section once it (and all
//! its descendant boundaries) are fully resolved.
//!
//! # Usage
//!
//! ```ignore
//! use mdx2md_core::streaming::StreamConverter;
//! use mdx2md_core::config::Config;
//!
//! let config = Config::default();
//! let mut converter = StreamConverter::new(config);
//!
//! for chunk in response_stream {
//!     converter.push(&chunk);
//!     for md in converter.drain() {
//!         print!("{}", md.content);
//!     }
//! }
//! // Flush any remaining content (unresolved boundaries use their fallback)
//! if let Some(md) = converter.finish() {
//!     print!("{}", md.content);
//! }
//! ```

use std::collections::HashMap;

use crate::config::Config;

/// A chunk of converted Markdown emitted by the stream converter.
#[derive(Debug, Clone, PartialEq)]
pub struct MarkdownChunk {
    /// The Markdown content.
    pub content: String,
    /// Whether this chunk came from a resolved Suspense boundary
    /// (vs. static content outside any boundary).
    pub from_boundary: bool,
}

/// Tracks the state of a single Suspense boundary.
#[derive(Debug)]
struct Boundary {
    /// Boundary template ID (e.g. "B:0").
    #[allow(dead_code)]
    id: String,
    /// Byte offset in `buffer` where the fallback content starts
    /// (after `<!--$?--><template id="B:X"></template>`).
    fallback_start: usize,
    /// Byte offset where the fallback ends (at `<!--/$-->`).
    fallback_end: Option<usize>,
    /// Resolved HTML content (from the `<div hidden id="S:X">` block).
    resolved_content: Option<String>,
    /// IDs of child boundaries nested inside this one.
    children: Vec<String>,
    /// Parent boundary ID, if nested.
    parent: Option<String>,
}

/// Streaming converter that buffers React SSR chunks and emits Markdown
/// as Suspense boundaries resolve.
pub struct StreamConverter {
    config: Config,
    /// Accumulated raw HTML from all `push()` calls.
    buffer: String,
    /// Scan cursor: how far into `buffer` we've already parsed for markers.
    scan_pos: usize,
    /// Suspense boundaries keyed by their template ID (e.g. "B:0").
    boundaries: HashMap<String, Boundary>,
    /// Mapping from slot ID ("S:X") to the resolved HTML content.
    slots: HashMap<String, String>,
    /// Boundary IDs in the order they appear in the document (for output ordering).
    boundary_order: Vec<String>,
    /// Markdown chunks ready to be drained.
    pending_output: Vec<MarkdownChunk>,
    /// Byte offset up to which we've already emitted static (non-boundary) content.
    emitted_up_to: usize,
    /// Boundary IDs that have already been emitted as Markdown.
    emitted_boundaries: Vec<String>,
    /// Stack of currently open boundary IDs during scanning.
    open_stack: Vec<String>,
}

impl StreamConverter {
    /// Create a new streaming converter with the given config.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            buffer: String::new(),
            scan_pos: 0,
            boundaries: HashMap::new(),
            slots: HashMap::new(),
            boundary_order: Vec::new(),
            pending_output: Vec::new(),
            emitted_up_to: 0,
            emitted_boundaries: Vec::new(),
            open_stack: Vec::new(),
        }
    }

    /// Feed a chunk of HTML into the converter.
    pub fn push(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        self.scan();
        self.try_emit();
    }

    /// Drain any Markdown chunks that are ready.
    pub fn drain(&mut self) -> Vec<MarkdownChunk> {
        std::mem::take(&mut self.pending_output)
    }

    /// Finalize the stream. Converts any remaining buffered content to Markdown.
    /// Unresolved boundaries use their fallback content.
    pub fn finish(mut self) -> Option<MarkdownChunk> {
        // Resolve any remaining unresolved boundaries with their fallback content
        let boundary_ids: Vec<String> = self.boundaries.keys().cloned().collect();
        for id in &boundary_ids {
            if self.boundaries[id].resolved_content.is_none() {
                if let (Some(start), Some(end)) = (
                    Some(self.boundaries[id].fallback_start),
                    self.boundaries[id].fallback_end,
                ) {
                    let fallback = self.buffer[start..end].to_string();
                    self.boundaries.get_mut(id).unwrap().resolved_content = Some(fallback);
                }
            }
        }

        // Build the final HTML with all resolutions applied
        let html = self.build_resolved_html(self.emitted_up_to);
        if html.trim().is_empty() {
            return None;
        }

        let md = self.html_to_markdown(&html);
        if md.trim().is_empty() {
            return None;
        }

        Some(MarkdownChunk {
            content: md,
            from_boundary: false,
        })
    }

    /// Scan newly appended buffer content for React streaming markers.
    ///
    /// Handles markers that arrive split across multiple `push()` calls by
    /// backing up `scan_pos` when a marker is found but can't be fully parsed.
    fn scan(&mut self) {
        loop {
            if self.scan_pos >= self.buffer.len() {
                break;
            }

            let rest = &self.buffer[self.scan_pos..];

            // Find the earliest interesting marker in the remaining buffer.
            let candidates: Vec<(&str, Option<usize>)> = vec![
                ("<!--$?-->", rest.find("<!--$?-->")),
                ("<!--/$-->", rest.find("<!--/$-->")),
                ("<div hidden id=\"S:", rest.find("<div hidden id=\"S:")),
                ("<div hidden id='S:", rest.find("<div hidden id='S:")),
                ("$RC(", rest.find("$RC(")),
            ];

            // Pick the earliest match
            let earliest = candidates
                .iter()
                .filter_map(|(tag, pos)| pos.map(|p| (*tag, p)))
                .min_by_key(|&(_, p)| p);

            let Some((tag, offset)) = earliest else {
                // No markers found. Back up scan_pos to catch markers that
                // might be split across the current buffer end.
                // The longest marker prefix is "<!--$?-->" (9 bytes).
                let backtrack = self.buffer.len().saturating_sub(20);
                self.scan_pos = self.scan_pos.max(backtrack);
                break;
            };

            let abs = self.scan_pos + offset;

            match tag {
                "<!--$?-->" => {
                    let after_marker = abs + "<!--$?-->".len();

                    if after_marker >= self.buffer.len() {
                        // Marker found but nothing after it yet
                        self.scan_pos = abs;
                        break;
                    }

                    // Try to extract <template id="B:X"></template>
                    match self.extract_boundary_template_id(after_marker) {
                        Some(bid) => {
                            match self.try_find_template_close(after_marker) {
                                Some(template_end) => {
                                    let parent = self.open_stack.last().cloned();

                                    let boundary = Boundary {
                                        id: bid.clone(),
                                        fallback_start: template_end,
                                        fallback_end: None,
                                        resolved_content: None,
                                        children: Vec::new(),
                                        parent: parent.clone(),
                                    };

                                    if let Some(ref parent_id) = parent {
                                        if let Some(p) = self.boundaries.get_mut(parent_id) {
                                            p.children.push(bid.clone());
                                        }
                                    }

                                    self.boundaries.insert(bid.clone(), boundary);
                                    self.boundary_order.push(bid.clone());
                                    self.open_stack.push(bid);
                                    self.scan_pos = template_end;
                                }
                                None => {
                                    // Template close tag hasn't arrived — back up and wait
                                    self.scan_pos = abs;
                                    break;
                                }
                            }
                        }
                        None => {
                            // Not a boundary template — skip past marker
                            self.scan_pos = after_marker;
                        }
                    }
                }

                "<!--/$-->" => {
                    if let Some(bid) = self.open_stack.pop() {
                        if let Some(b) = self.boundaries.get_mut(&bid) {
                            b.fallback_end = Some(abs);
                        }
                    }
                    self.scan_pos = abs + "<!--/$-->".len();
                }

                "<div hidden id=\"S:" => {
                    if let Some((slot_id, content, end_pos)) = self.extract_hidden_slot(abs) {
                        self.slots.insert(slot_id, content);
                        self.scan_pos = end_pos;
                    } else {
                        // Incomplete — wait for more data
                        self.scan_pos = abs;
                        break;
                    }
                }

                "<div hidden id='S:" => {
                    if let Some((slot_id, content, end_pos)) =
                        self.extract_hidden_slot_single_quote(abs)
                    {
                        self.slots.insert(slot_id, content);
                        self.scan_pos = end_pos;
                    } else {
                        self.scan_pos = abs;
                        break;
                    }
                }

                "$RC(" => {
                    if let Some((bid, sid, end_pos)) = self.extract_rc_call(abs) {
                        if let Some(content) = self.slots.get(&sid).cloned() {
                            if let Some(b) = self.boundaries.get_mut(&bid) {
                                b.resolved_content = Some(content);
                            }
                        }
                        self.scan_pos = end_pos;
                    } else {
                        self.scan_pos = abs;
                        break;
                    }
                }

                _ => {
                    self.scan_pos = abs + 1;
                }
            }
        }
    }

    /// Try to emit Markdown for fully-resolved sections.
    fn try_emit(&mut self) {
        // Emit static content before the first boundary
        if !self.boundary_order.is_empty() {
            let first = &self.boundary_order[0];
            if let Some(b) = self.boundaries.get(first) {
                // Find the start of the boundary marker in the buffer
                let marker = "<!--$?-->".to_string();
                if let Some(marker_pos) =
                    self.buffer[self.emitted_up_to..b.fallback_start].rfind(&marker)
                {
                    let abs_pos = self.emitted_up_to + marker_pos;
                    if abs_pos > self.emitted_up_to {
                        let static_html = &self.buffer[self.emitted_up_to..abs_pos];
                        if !static_html.trim().is_empty() {
                            let md = self.html_to_markdown(static_html);
                            if !md.trim().is_empty() {
                                self.pending_output.push(MarkdownChunk {
                                    content: md,
                                    from_boundary: false,
                                });
                            }
                        }
                        self.emitted_up_to = abs_pos;
                    }
                }
            }
        }

        // Check each top-level boundary for full resolution
        let top_level: Vec<String> = self
            .boundary_order
            .iter()
            .filter(|id| self.boundaries.get(*id).is_some_and(|b| b.parent.is_none()))
            .cloned()
            .collect();

        for bid in &top_level {
            if self.emitted_boundaries.contains(bid) {
                continue;
            }
            if self.is_fully_resolved(bid) {
                let html = self.build_boundary_html(bid);
                let md = self.html_to_markdown(&html);
                if !md.trim().is_empty() {
                    self.pending_output.push(MarkdownChunk {
                        content: md,
                        from_boundary: true,
                    });
                }
                self.emitted_boundaries.push(bid.clone());

                // Advance emitted_up_to past this boundary
                if let Some(b) = self.boundaries.get(bid) {
                    if let Some(end) = b.fallback_end {
                        let boundary_end = end + "<!--/$-->".len();
                        if boundary_end > self.emitted_up_to {
                            self.emitted_up_to = boundary_end;
                        }
                    }
                }
            }
        }
    }

    /// Check whether a boundary and all its descendants are resolved.
    fn is_fully_resolved(&self, bid: &str) -> bool {
        let Some(b) = self.boundaries.get(bid) else {
            return false;
        };
        if b.resolved_content.is_none() || b.fallback_end.is_none() {
            return false;
        }
        b.children.iter().all(|c| self.is_fully_resolved(c))
    }

    /// Build the final HTML for a resolved boundary, recursively substituting
    /// child boundaries.
    fn build_boundary_html(&self, bid: &str) -> String {
        let Some(b) = self.boundaries.get(bid) else {
            return String::new();
        };
        let mut html = b.resolved_content.as_deref().unwrap_or("").to_string();

        // Recursively replace child boundaries within this content
        for child_id in &b.children {
            if let Some(child) = self.boundaries.get(child_id) {
                // The child boundary marker pattern that might appear in the resolved content
                let child_html = self.build_boundary_html(child_id);
                if let Some(ref resolved) = child.resolved_content {
                    // Replace any remaining boundary markers for this child
                    let fallback_pattern = if let (Some(_start), Some(end)) =
                        (Some(child.fallback_start), child.fallback_end)
                    {
                        Some(self.buffer[child.fallback_start..end].to_string())
                    } else {
                        None
                    };
                    if let Some(ref fallback) = fallback_pattern {
                        html = html.replace(fallback, &child_html);
                    }
                    // Also try replacing the resolved content reference
                    html = html.replace(resolved.as_str(), &child_html);
                }
            }
        }

        html
    }

    /// Build resolved HTML for remaining content starting from a position.
    fn build_resolved_html(&self, from: usize) -> String {
        let mut html = self.buffer[from..].to_string();

        // Strip React streaming artifacts
        html = html.replace("<!--$?-->", "");
        html = html.replace("<!--/$-->", "");

        // Remove <template id="B:X"></template> tags
        while let Some(start) = html.find("<template id=") {
            if let Some(end) = html[start..].find("</template>") {
                html.replace_range(start..start + end + "</template>".len(), "");
            } else if let Some(end) = html[start..].find("/>") {
                html.replace_range(start..start + end + "/>".len(), "");
            } else {
                break;
            }
        }

        // Remove <div hidden id="S:X">...</div> blocks
        while let Some(start) = html.find("<div hidden id=") {
            if let Some(end) = html[start..].find("</div>") {
                html.replace_range(start..start + end + "</div>".len(), "");
            } else {
                break;
            }
        }

        // Remove <script>$RC...</script> blocks
        while let Some(start) = html.find("<script>") {
            if let Some(end) = html[start..].find("</script>") {
                html.replace_range(start..start + end + "</script>".len(), "");
            } else {
                break;
            }
        }

        // Apply boundary resolutions
        for bid in &self.boundary_order {
            if let Some(b) = self.boundaries.get(bid) {
                if let Some(ref resolved) = b.resolved_content {
                    if let Some(end) = b.fallback_end {
                        let fallback = &self.buffer[b.fallback_start..end];
                        html = html.replace(fallback, resolved);
                    }
                }
            }
        }

        html
    }

    /// Convert an HTML fragment to Markdown using the existing mdx2md pipeline.
    fn html_to_markdown(&self, html: &str) -> String {
        let cleaned = clean_html_for_conversion(html);
        if cleaned.trim().is_empty() {
            return String::new();
        }

        // Use the core convert pipeline — it handles HTML tags via component
        // templates and Markdown passthrough.
        match crate::convert(&cleaned, &self.config) {
            Ok(md) => md,
            Err(_) => {
                // If MDX parsing fails (e.g. unclosed tags in a partial chunk),
                // pass through as-is — the rewriter can still strip comments/doctype.
                crate::rewriter::rewrite_markdown(&cleaned, &self.config)
            }
        }
    }

    // ── Marker extraction helpers ──────────────────────────────────────

    /// Extract the boundary ID from `<template id="B:X"></template>`.
    fn extract_boundary_template_id(&self, pos: usize) -> Option<String> {
        let rest = &self.buffer[pos..];
        let prefix_dq = "<template id=\"";
        let prefix_sq = "<template id='";

        let (after, quote) = if let Some(stripped) = rest.strip_prefix(prefix_dq) {
            (stripped, b'"')
        } else {
            let stripped = rest.strip_prefix(prefix_sq)?;
            (stripped, b'\'')
        };

        let end = after.find(quote as char)?;
        let id = &after[..end];
        if id.starts_with("B:") {
            Some(id.to_string())
        } else {
            None
        }
    }

    /// Find the end of `<template ...></template>` or `<template .../>`.
    /// Returns `None` if the close tag hasn't arrived yet.
    fn try_find_template_close(&self, pos: usize) -> Option<usize> {
        let rest = &self.buffer[pos..];
        if let Some(offset) = rest.find("</template>") {
            Some(pos + offset + "</template>".len())
        } else {
            rest.find("/>").map(|offset| pos + offset + "/>".len())
        }
    }

    /// Extract a hidden slot: `<div hidden id="S:X">content</div>`.
    fn extract_hidden_slot(&self, pos: usize) -> Option<(String, String, usize)> {
        let rest = &self.buffer[pos..];
        let prefix = "id=\"S:";
        let id_start = rest.find(prefix)? + prefix.len();
        let id_end = rest[id_start..].find('"')?;
        let slot_id = format!("S:{}", &rest[id_start..id_start + id_end]);

        // Find the closing > of the opening div
        let content_start = rest.find('>')? + 1;

        // Find matching </div> — handle nesting
        let content_rest = &rest[content_start..];
        let close_pos = find_matching_div_close(content_rest)?;
        let content = content_rest[..close_pos].to_string();
        let end = pos + content_start + close_pos + "</div>".len();

        Some((slot_id, content, end))
    }

    /// Same as above but for single-quoted attributes.
    fn extract_hidden_slot_single_quote(&self, pos: usize) -> Option<(String, String, usize)> {
        let rest = &self.buffer[pos..];
        let prefix = "id='S:";
        let id_start = rest.find(prefix)? + prefix.len();
        let id_end = rest[id_start..].find('\'')?;
        let slot_id = format!("S:{}", &rest[id_start..id_start + id_end]);

        let content_start = rest.find('>')? + 1;
        let content_rest = &rest[content_start..];
        let close_pos = find_matching_div_close(content_rest)?;
        let content = content_rest[..close_pos].to_string();
        let end = pos + content_start + close_pos + "</div>".len();

        Some((slot_id, content, end))
    }

    /// Extract a `$RC("B:X","S:X")` call.
    fn extract_rc_call(&self, pos: usize) -> Option<(String, String, usize)> {
        let rest = &self.buffer[pos..];
        if !rest.starts_with("$RC(") {
            return None;
        }

        let inner_start = 4; // past "$RC("
        let inner_end = rest.find(')')?;
        let inner = &rest[inner_start..inner_end];

        // Parse two quoted arguments: "B:X","S:X" or 'B:X','S:X'
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() != 2 {
            return None;
        }

        let bid = parts[0].trim().trim_matches(|c| c == '"' || c == '\'');
        let sid = parts[1].trim().trim_matches(|c| c == '"' || c == '\'');

        Some((bid.to_string(), sid.to_string(), pos + inner_end + 1))
    }
}

/// Find the matching `</div>` for content that starts inside a `<div>`.
/// Handles nested `<div>` tags.
fn find_matching_div_close(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut pos = 0;
    let bytes = s.as_bytes();

    while pos < bytes.len() {
        if s[pos..].starts_with("</div>") {
            if depth == 0 {
                return Some(pos);
            }
            depth -= 1;
            pos += 6;
        } else if s[pos..].starts_with("<div") {
            // Check it's actually a div tag (not `<divider>` etc.)
            let after = pos + 4;
            if after < bytes.len()
                && (bytes[after] == b' '
                    || bytes[after] == b'>'
                    || bytes[after] == b'/'
                    || bytes[after] == b'\n')
            {
                depth += 1;
                pos += 4;
            } else {
                pos += 1;
            }
        } else {
            pos += 1;
        }
    }
    None
}

/// Clean up raw HTML for conversion through the mdx2md pipeline.
fn clean_html_for_conversion(html: &str) -> String {
    let mut s = html.to_string();

    // Strip <script>...</script> blocks
    while let Some(start) = s.find("<script") {
        if let Some(end) = s[start..].find("</script>") {
            s.replace_range(start..start + end + "</script>".len(), "");
        } else {
            break;
        }
    }

    // Strip <style>...</style> blocks
    while let Some(start) = s.find("<style") {
        if let Some(end) = s[start..].find("</style>") {
            s.replace_range(start..start + end + "</style>".len(), "");
        } else {
            break;
        }
    }

    // Strip React streaming comment markers
    s = s.replace("<!--$-->", "");
    s = s.replace("<!--$?-->", "");
    s = s.replace("<!--/$-->", "");
    s = s.replace("<!-- -->", "");

    // Strip <template> tags (used by Suspense)
    while let Some(start) = s.find("<template") {
        if let Some(end) = s[start..].find("</template>") {
            s.replace_range(start..start + end + "</template>".len(), "");
        } else if let Some(end) = s[start..].find("/>") {
            s.replace_range(start..start + end + "/>".len(), "");
        } else {
            break;
        }
    }

    // Strip <div hidden ...>...</div> (resolution slots)
    while let Some(start) = s.find("<div hidden") {
        if let Some(content_start_rel) = s[start..].find('>') {
            let content_start = start + content_start_rel + 1;
            if let Some(close_rel) = find_matching_div_close(&s[content_start..]) {
                let end = content_start + close_rel + "</div>".len();
                s.replace_range(start..end, "");
            } else {
                break;
            }
        } else {
            break;
        }
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> Config {
        Config::default()
    }

    #[test]
    fn test_static_html_no_suspense() {
        let mut conv = StreamConverter::new(default_config());
        conv.push("<h1>Hello</h1><p>World</p>");
        let chunks = conv.drain();
        let final_chunk = conv.finish();

        let all: String = chunks
            .iter()
            .chain(final_chunk.iter())
            .map(|c| c.content.as_str())
            .collect();

        assert!(
            all.contains("Hello") && all.contains("World"),
            "Should convert static HTML. Got: {all}"
        );
    }

    #[test]
    fn test_single_boundary_resolved() {
        let mut conv = StreamConverter::new(default_config());

        // Initial shell with a Suspense boundary
        conv.push(
            r#"<h1>Title</h1><!--$?--><template id="B:0"></template><p>Loading...</p><!--/$-->"#,
        );

        // Nothing should be emitted for the boundary yet
        let chunks = conv.drain();
        assert!(
            chunks.iter().all(|c| !c.from_boundary),
            "Boundary should not be emitted before resolution"
        );

        // Resolution arrives
        conv.push(r#"<div hidden id="S:0"><p>Resolved content!</p></div>"#);
        conv.push(r#"<script>$RC("B:0","S:0")</script>"#);

        let chunks = conv.drain();
        let final_chunk = conv.finish();
        let all: String = chunks
            .iter()
            .chain(final_chunk.iter())
            .map(|c| c.content.as_str())
            .collect();

        assert!(
            all.contains("Resolved content!"),
            "Should contain resolved content. Got: {all}"
        );
        assert!(
            !all.contains("Loading..."),
            "Should not contain fallback. Got: {all}"
        );
    }

    #[test]
    fn test_out_of_order_resolution() {
        let mut conv = StreamConverter::new(default_config());

        // Two boundaries: B:0 (outer) contains B:1 (inner)
        conv.push(concat!(
            "<h1>Page</h1>",
            "<!--$?--><template id=\"B:0\"></template>",
            "<p>Outer loading</p>",
            "<!--$?--><template id=\"B:1\"></template>",
            "<p>Inner loading</p>",
            "<!--/$-->",
            "<!--/$-->",
        ));

        // Inner resolves first (out of order)
        conv.push(r#"<div hidden id="S:1"><p>Inner done</p></div>"#);
        conv.push(r#"<script>$RC("B:1","S:1")</script>"#);

        // Outer should NOT be emitted yet
        let chunks = conv.drain();
        let boundary_chunks: Vec<_> = chunks.iter().filter(|c| c.from_boundary).collect();
        assert!(
            boundary_chunks.is_empty(),
            "Outer boundary should wait for full resolution"
        );

        // Now outer resolves
        conv.push(r#"<div hidden id="S:0"><p>Outer done</p></div>"#);
        conv.push(r#"<script>$RC("B:0","S:0")</script>"#);

        let chunks = conv.drain();
        let final_chunk = conv.finish();
        let all: String = chunks
            .iter()
            .chain(final_chunk.iter())
            .map(|c| c.content.as_str())
            .collect();

        assert!(
            all.contains("Outer done") || all.contains("Inner done"),
            "Should contain resolved content. Got: {all}"
        );
        assert!(
            !all.contains("loading"),
            "Should not contain fallback text. Got: {all}"
        );
    }

    #[test]
    fn test_finish_uses_fallback_for_unresolved() {
        let mut conv = StreamConverter::new(default_config());

        conv.push(r#"<!--$?--><template id="B:0"></template><p>Fallback content</p><!--/$-->"#);

        // Never resolve — finish() should use fallback
        let _ = conv.drain();
        let final_chunk = conv.finish();
        let content = final_chunk.map(|c| c.content).unwrap_or_default();

        assert!(
            content.contains("Fallback content"),
            "finish() should use fallback for unresolved boundaries. Got: {content}"
        );
    }

    #[test]
    fn test_rc_call_parsing() {
        let mut conv = StreamConverter::new(default_config());
        conv.buffer = r#"$RC("B:0","S:0")"#.to_string();
        let result = conv.extract_rc_call(0);
        assert_eq!(result, Some(("B:0".to_string(), "S:0".to_string(), 16)));
    }

    #[test]
    fn test_rc_call_single_quotes() {
        let mut conv = StreamConverter::new(default_config());
        conv.buffer = "$RC('B:2','S:2')".to_string();
        let result = conv.extract_rc_call(0);
        assert_eq!(result, Some(("B:2".to_string(), "S:2".to_string(), 16)));
    }

    #[test]
    fn test_hidden_slot_extraction() {
        let mut conv = StreamConverter::new(default_config());
        conv.buffer = r#"<div hidden id="S:0"><p>Hello</p></div>"#.to_string();
        let result = conv.extract_hidden_slot(0);
        assert!(result.is_some());
        let (id, content, _) = result.unwrap();
        assert_eq!(id, "S:0");
        assert_eq!(content, "<p>Hello</p>");
    }

    #[test]
    fn test_hidden_slot_nested_divs() {
        let mut conv = StreamConverter::new(default_config());
        conv.buffer =
            r#"<div hidden id="S:0"><div class="wrapper"><p>Deep</p></div></div>"#.to_string();
        let result = conv.extract_hidden_slot(0);
        assert!(result.is_some());
        let (id, content, _) = result.unwrap();
        assert_eq!(id, "S:0");
        assert_eq!(content, r#"<div class="wrapper"><p>Deep</p></div>"#);
    }

    #[test]
    fn test_boundary_template_id_extraction() {
        let mut conv = StreamConverter::new(default_config());
        conv.buffer = r#"<template id="B:0"></template>"#.to_string();
        let result = conv.extract_boundary_template_id(0);
        assert_eq!(result, Some("B:0".to_string()));
    }

    #[test]
    fn test_find_matching_div_close() {
        assert_eq!(find_matching_div_close("<p>simple</p></div>"), Some(13));
        assert_eq!(find_matching_div_close("<div>nested</div></div>"), Some(17));
        assert_eq!(find_matching_div_close("no close tag"), None);
    }

    #[test]
    fn test_clean_html_strips_scripts() {
        let html = "<p>Hello</p><script>$RC('B:0','S:0')</script><p>World</p>";
        let cleaned = clean_html_for_conversion(html);
        assert!(!cleaned.contains("$RC"));
        assert!(cleaned.contains("Hello"));
        assert!(cleaned.contains("World"));
    }

    #[test]
    fn test_incremental_chunks() {
        let mut conv = StreamConverter::new(default_config());

        // Feed the HTML in small incremental chunks
        conv.push("<h1>Ti");
        conv.push("tle</h1>");
        conv.push("<!--$?--><template id=\"B:0\">");
        conv.push("</template><p>Loading</p><!--/$-->");
        conv.push("<div hidden id=\"S:0\">");
        conv.push("<p>Done!</p></div>");
        conv.push("<script>$RC(\"B:0\",\"S:0\")</script>");

        let chunks = conv.drain();
        let final_chunk = conv.finish();
        let all: String = chunks
            .iter()
            .chain(final_chunk.iter())
            .map(|c| c.content.as_str())
            .collect();

        assert!(all.contains("Done!"), "Should resolve. Got: {all}");
    }
}
