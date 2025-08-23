#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
OUT_DIR="$ROOT_DIR/docs/dep-graphs"
SRC_TAURI_DIR="$ROOT_DIR/src-tauri"
CARGO_TOML="$SRC_TAURI_DIR/Cargo.toml"
mkdir -p "$OUT_DIR"

# Args: --max-depth N, --focus-on MOD_PATH, --focus-dir REL_OR_ABS_PATH
MAX_DEPTH=""
FOCUS_ON=""
FOCUS_DIR=""
BIN_NAME=""
WITH_CRATE_GRAPH=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --max-depth)
      MAX_DEPTH="$2"; shift 2 ;;
    --focus-on)
      FOCUS_ON="$2"; shift 2 ;;
    --focus-dir)
      FOCUS_DIR="$2"; shift 2 ;;
    --bin)
      BIN_NAME="$2"; shift 2 ;;
    --crate-graph|--with-crate-graph)
      WITH_CRATE_GRAPH=1; shift ;;
    *)
      echo "[warn] Unknown arg: $1"; shift ;;
  esac
done

# Detect package name from src-tauri/Cargo.toml
PKG_NAME=""
if [[ -f "$CARGO_TOML" ]]; then
  PKG_NAME=$(grep -E '^[[:space:]]*name[[:space:]]*=' "$CARGO_TOML" | head -n1 | sed -E 's/.*"([^"]+)".*/\1/') || true
fi
if [[ -z "$PKG_NAME" ]]; then
  echo "[warn] Could not detect package name from $CARGO_TOML; will try without -p"
else
  echo "[info] Detected package name: $PKG_NAME"
fi

# Produce structure trees to help choose focus paths
echo "[modules] Emitting module tree (structure) to help focus selection"
if command -v cargo-modules >/dev/null 2>&1; then
  set +e
  struct_err="$OUT_DIR/modules_structure.err"
  if [[ -n "$PKG_NAME" ]]; then
    (cd "$SRC_TAURI_DIR" && cargo modules structure --lib -p "$PKG_NAME" --all-features > "$OUT_DIR/modules_structure_lib_raw.txt" 2>"$struct_err")
    s_status=$?
  else
    s_status=1
  fi
  if [[ $s_status -ne 0 ]]; then
    (cd "$SRC_TAURI_DIR" && cargo modules structure --lib --all-features > "$OUT_DIR/modules_structure_lib_raw.txt" 2>>"$struct_err")
    s_status=$?
  fi
  # Produce ASCII-friendly version
  if [[ -s "$OUT_DIR/modules_structure_lib_raw.txt" ]]; then
    # Strip ANSI color codes then convert box-drawing to ASCII
    if command -v perl >/dev/null 2>&1; then
      perl -pe 's/\e\[[0-9;]*m//g' "$OUT_DIR/modules_structure_lib_raw.txt" | \
      sed -e 's/├── /+-- /g' \
          -e 's/└── /`-- /g' \
          -e 's/│   /|   /g' \
          -e 's/│/|/g' \
          -e 's/─/-/g' > "$OUT_DIR/modules_structure_lib.txt" || cp "$OUT_DIR/modules_structure_lib_raw.txt" "$OUT_DIR/modules_structure_lib.txt"
    else
      sed -e 's/\x1b\[[0-9;]*m//g' \
          -e 's/├── /+-- /g' \
          -e 's/└── /`-- /g' \
          -e 's/│   /|   /g' \
          -e 's/│/|/g' \
          -e 's/─/-/g' "$OUT_DIR/modules_structure_lib_raw.txt" > "$OUT_DIR/modules_structure_lib.txt" || cp "$OUT_DIR/modules_structure_lib_raw.txt" "$OUT_DIR/modules_structure_lib.txt"
    fi
  fi
  if [[ -n "$BIN_NAME" ]]; then
    # best-effort binary structure dump
    (cd "$SRC_TAURI_DIR" && cargo modules structure --bin "$BIN_NAME" --all-features > "$OUT_DIR/modules_structure_bin_${BIN_NAME}_raw.txt" 2>>"$struct_err")
    if [[ -s "$OUT_DIR/modules_structure_bin_${BIN_NAME}_raw.txt" ]]; then
      sed -e 's/├── /+-- /g' \
          -e 's/└── /`-- /g' \
          -e 's/│   /|   /g' \
          -e 's/│/|/g' \
          -e 's/─/-/g' "$OUT_DIR/modules_structure_bin_${BIN_NAME}_raw.txt" > "$OUT_DIR/modules_structure_bin_${BIN_NAME}.txt" || cp "$OUT_DIR/modules_structure_bin_${BIN_NAME}_raw.txt" "$OUT_DIR/modules_structure_bin_${BIN_NAME}.txt"
    fi
  fi
  set -e
else
  echo "[modules] cargo-modules not installed; skipping structure tree"
fi

if [[ $WITH_CRATE_GRAPH -eq 1 ]]; then
  echo "[depgraph] Generating crate dependency graph (cargo-depgraph)"
  if command -v cargo-depgraph >/dev/null 2>&1; then
    # cargo-depgraph expects a --root <pkg> (no --dot option)
    if [[ -z "$PKG_NAME" ]]; then
      echo "[depgraph] ERROR: Package name is unknown; cannot run cargo depgraph --root <pkg>." > "$OUT_DIR/crate-deps.err"
    elif (cd "$SRC_TAURI_DIR" && cargo depgraph --root "$PKG_NAME" > "$OUT_DIR/crate-deps.dot" 2>"$OUT_DIR/crate-deps.err"); then
      echo "[depgraph] Wrote $OUT_DIR/crate-deps.dot"
      if command -v dot >/dev/null 2>&1; then
        if dot -Tsvg "$OUT_DIR/crate-deps.dot" -o "$OUT_DIR/crate-deps.svg"; then
          echo "[depgraph] Wrote $OUT_DIR/crate-deps.svg"
        else
          echo "[depgraph] dot rendering failed (crate-deps). See Graphviz install?"
        fi
      else
        echo "[depgraph] Graphviz 'dot' not found; skipped SVG rendering"
      fi
    else
      echo "[depgraph] cargo depgraph failed. See $OUT_DIR/crate-deps.err"
    fi
  else
    echo "[depgraph] cargo-depgraph not installed. Run scripts/install_dep_vis_tools.sh"
  fi
else
  echo "[depgraph] Skipping crate graph (enable with --crate-graph)"
fi

echo "[modules] Generating module graph (cargo-modules dependencies)"
if command -v cargo-modules >/dev/null 2>&1; then
  # Try with detected package name first
  set +e
  # If focus-dir provided, translate to module path (rough heuristic)
  if [[ -n "$FOCUS_DIR" && -z "$FOCUS_ON" ]]; then
    dir="$FOCUS_DIR"
    # Normalize absolute path to relative within src-tauri/src
    dir="${dir#$ROOT_DIR/}"
    dir="${dir#$SRC_TAURI_DIR/}"
    dir="${dir#src/}"
    dir="${dir#src-tauri/src/}"
    dir="${dir%.rs}"
    dir="${dir#/}"
    # Convert / to :: for module path
  FOCUS_ON="${dir//\//::}"
    # Validate module-like pattern (ident(::ident)*) else drop it
    if [[ ! "$FOCUS_ON" =~ ^[A-Za-z0-9_]+(::[A-Za-z0-9_]+)*$ ]]; then
      echo "[modules] Focus-dir '$FOCUS_DIR' does not map to a valid module path; skipping --focus-on."
      FOCUS_ON=""
    else
      echo "[modules] Focus derived from directory (heuristic): $FOCUS_DIR -> $FOCUS_ON"
    fi
  fi

  # Flags and outputs will be (re)built after any focus mapping below
  cm_flags=()
  target_suffix="lib"
  out_suffix=""
  modules_dot="$OUT_DIR/modules_lib.dot"
  modules_svg="$OUT_DIR/modules_lib.svg"
  modules_err="$OUT_DIR/modules_lib.err"

  # If focus-dir provided, try crate-prefixed and bare module candidates before filtered fallback
  if [[ -n "$FOCUS_DIR" ]]; then
    # Build a filtered graph by the last path segment to avoid empty focus graphs
    last_segment="$(basename "${FOCUS_DIR%/}")"
    struct_file="$OUT_DIR/modules_structure_lib.txt"
    [[ -n "$BIN_NAME" ]] && struct_file="$OUT_DIR/modules_structure_bin_${BIN_NAME}.txt"
    echo "[modules] Focus-dir '$FOCUS_DIR' -> segment '$last_segment'. Building filtered graph..." | (tee "$OUT_DIR/modules_lib.err" >/dev/null || true)

    probe_full="$OUT_DIR/.modules_full_probe.dot"
    cm_probe_flags=(dependencies --all-features --no-externs --no-fns --no-traits --no-types --no-uses)
    if [[ -n "$BIN_NAME" ]]; then cm_probe_flags+=(--bin "$BIN_NAME"); else cm_probe_flags+=(--lib); fi
    if [[ -n "$PKG_NAME" ]]; then
      (cd "$SRC_TAURI_DIR" && cargo modules "${cm_probe_flags[@]}" -p "$PKG_NAME" > "$probe_full" 2>>"$OUT_DIR/modules_lib.err") || true
    else
      (cd "$SRC_TAURI_DIR" && cargo modules "${cm_probe_flags[@]}" > "$probe_full" 2>>"$OUT_DIR/modules_lib.err") || true
    fi
    if [[ -s "$probe_full" ]]; then
      keep_list="$OUT_DIR/.modules_keep.txt"
      # Extract all quoted node/edge names from DOT and keep those with the segment
      grep -oE '"[^"]+"' "$probe_full" | sed -E 's/^"|"$//g' | \
        grep -E "(^|::)${last_segment}(::|$)" | sort -u > "$keep_list" || true
      if [[ -s "$keep_list" ]]; then
        modules_dot="$OUT_DIR/modules_lib_focus_${last_segment}_filtered.dot"
        modules_svg="$OUT_DIR/modules_lib_focus_${last_segment}_filtered.svg"
        probe_clean="$OUT_DIR/.modules_full_probe.clean.dot"
        sed -E 's@[[:space:]]*//.*$@@' "$probe_full" > "$probe_clean"
    awk -v keepfile="$keep_list" '
          BEGIN {
            while ((getline k < keepfile) > 0) {
              sub(/^\s+|\s+$/, "", k); if (k != "") { keys[++n]=k; keep[k]=1 }
            }
          }
          {
            # Always keep header/footer
      if ($0 ~ /^[[:space:]]*digraph\b/ || $0 ~ /^[[:space:]]*}[[:space:]]*$/) { print $0; next }
            # Keep any line without quotes
            if ($0 !~ /"/) { print $0; next }
            # Keep if it contains any kept module name in quotes
            for (i=1; i<=n; i++) {
              k = keys[i]
              if (index($0, "\"" k "\"") > 0) { print $0; next }
            }
            # else drop
          }
        ' "$probe_clean" > "$modules_dot"
        # Post-process: apply depth-based coloring and add unreferenced list panel
        base_full="matter_certis_v2_lib::${last_segment}"
        # Recolor node fillcolor by relative depth from the focus segment
        colors="#1f77b4,#2ca02c,#ff7f0e,#9467bd,#17becf,#d62728,#8c564b"
        awk -v colors="$colors" -v base="${base_full}" '
          BEGIN { plen = split(colors, pal, ",") }
          function relDepth(name, base,    idx, tail, n) {
            if (name == base) return 0
            if (index(name, base "::") != 1) return 0
            tail = substr(name, length(base)+3)
            if (tail == "") return 0
            n = split(tail, tmp, /::/)
            return n
          }
          # inject global node style=filled
          /^[[:space:]]*node \[/ { print; print "        style=\"filled\","; next }
          # recolor node lines
          /^[[:space:]]*"[^"]+"[[:space:]]*\[.*\];/ {
            match($0, /"[^"]+"/); name=substr($0, RSTART+1, RLENGTH-2)
            d = relDepth(name, base)
            idx = (d % plen); if (idx==0) idx=plen
            fc = pal[idx]
            line=$0
            if (line ~ /fillcolor="[^"]*"/) {
              gsub(/fillcolor="[^"]*"/, "fillcolor=\"" fc "\"", line)
            } else {
              sub(/\];/, ", fillcolor=\"" fc "\" ];", line)
            }
            print line
            next
          }
          { print }
        ' "$modules_dot" > "$modules_dot.tmp" && mv "$modules_dot.tmp" "$modules_dot"

        # Build a uses-probe to detect unreferenced (no incoming uses edges) modules within the focus segment
        uses_probe="$OUT_DIR/.modules_uses_probe.dot"
        cm_uses_flags=(dependencies --all-features --no-externs --no-fns --no-traits --no-types)
        if [[ -n "$BIN_NAME" ]]; then cm_uses_flags+=(--bin "$BIN_NAME"); else cm_uses_flags+=(--lib); fi
        if [[ -n "$PKG_NAME" ]]; then
          (cd "$SRC_TAURI_DIR" && cargo modules "${cm_uses_flags[@]}" -p "$PKG_NAME" > "$uses_probe" 2>>"$OUT_DIR/modules_lib.err") || true
        else
          (cd "$SRC_TAURI_DIR" && cargo modules "${cm_uses_flags[@]}" > "$uses_probe" 2>>"$OUT_DIR/modules_lib.err") || true
        fi
    orphan_list_file="$OUT_DIR/.modules_orphans.txt"
    awk -v keepfile="$keep_list" -v seg="${last_segment}" '
          BEGIN {
            while ((getline k < keepfile) > 0) { keep[k]=1; indeg[k]=0 }
          }
          # match "a" -> "b" edges that are labeled uses (if labels exist) or any edge; we’ll prefer those containing "uses"
          /"[^"]+"\s*->\s*"[^"]+"/ {
            line=$0
            match(line, /"[^"]+"/); a=substr(line, RSTART+1, RLENGTH-2)
            rest=substr(line, RSTART+RLENGTH)
            match(rest, /"[^"]+"/); b=substr(rest, RSTART+1, RLENGTH-2)
      if (b in keep) { indeg[b]++ }
          }
          END {
            for (k in keep) {
              if (k ~ ("(::" seg "(::|$))") ) {
                # exclude the segment root itself from orphan detection
                if (k == "matter_certis_v2_lib::" seg) continue
                if (indeg[k] == 0) print k
              }
            }
          }
        ' "$uses_probe" | sort > "$orphan_list_file" || true

        if [[ -s "$orphan_list_file" ]]; then
          # Build a single panel listing unreferenced modules at bottom-right (rank=sink)
          orphan_panel="$OUT_DIR/.modules_orphans_panel.dot"
          : > "$orphan_panel"
          echo "  subgraph cluster_orphans {" >> "$orphan_panel"
          echo "      label=\"\"; style=\"dashed\"; color=\"#999999\";" >> "$orphan_panel"
          echo "      rank=sink;" >> "$orphan_panel"
          # Compose label
          printf '      "__orphans_panel__" [shape="box", style="filled", fillcolor="#f5f5f5", color="#999999", fontname="monospace", fontsize="10", label="Unreferenced within %s\\l' "${last_segment}" >> "$orphan_panel"
          while IFS= read -r mod; do
            short="$mod"
            short="${short#*::${last_segment}::}"
            if [[ "$short" == "$mod" ]]; then short="${short#*::${last_segment}}"; fi
            printf ' - %s\\l' "$short" >> "$orphan_panel"
          done < "$orphan_list_file"
          echo '" ];' >> "$orphan_panel"
          echo "  }" >> "$orphan_panel"
          echo "  \"${base_full}\" -> \"__orphans_panel__\" [style=\"invis\", constraint=false];" >> "$orphan_panel"
          # Insert the orphan panel before the final closing brace of the digraph (BSD awk compatible)
          awk 'FNR==NR { panel[++pn]=$0; next } {
                 if (NR==1) d=0
                 oc = gsub(/\{/, "&")
                 cc = gsub(/\}/, "&")
                 d += oc
                 if ($0 ~ /^\s*}\s*$/ && d <= 1) {
                   for(i=1;i<=pn;i++) print panel[i]
                   print $0; next
                 }
                 d -= cc; print
               }' "$orphan_panel" "$modules_dot" > "$modules_dot.tmp" && mv "$modules_dot.tmp" "$modules_dot"
        fi

        # Emit JSON for interactive viewer
        json_specific="$OUT_DIR/modules_lib_focus_${last_segment}_filtered.json"
        json_latest="$OUT_DIR/graph_focus.json"
        awk -v base="$base_full" '
          function relDepth(name, base,    tail,n){ if (name==base) return 0; if (index(name, base "::")!=1) return 0; tail=substr(name,length(base)+3); if (tail=="") return 0; n=split(tail,tmp,/::/); return n }
          function jsonEscape(s,    t){
            t=s
            # Convert Graphviz left-justified line breaks (\l) to newlines first
            gsub(/\\l/, "\n", t)
            # Then JSON-escape
            gsub(/\\/, "\\\\", t)
            gsub(/\"/, "\\\"", t)
            gsub(/\t/, "\\t", t)
            gsub(/\r/, "\\r", t)
            gsub(/\n/, "\\n", t)
            return t
          }
          BEGIN { print "{\n  \"base\": \"" base "\",\n  \"nodes\": ["; first=1 }
          /^[[:space:]]*"[^"]+"[[:space:]]*\[/ {
            match($0,/"[^"]+"/); name=substr($0,RSTART+1,RLENGTH-2)
            lbl=name
            if (match($0,/label=\"[^\"]*\"/)) { lbl=substr($0,RSTART+7,RLENGTH-8) }
            g=""; if (index(name, base "::")==1) { tail=substr(name,length(base)+3); split(tail,parts,/::/); if (length(parts[1])>0) g=parts[1] }
            d=relDepth(name, base)
            # JSON-escape label safely
            lbl = jsonEscape(lbl)
            # Also escape id if ever needed (defensive)
            idv = name; gsub(/\\/, "\\\\", idv); gsub(/\"/, "\\\"", idv)
            if (!first) printf ",\n"; first=0
            printf "    {\"id\": \"%s\", \"label\": \"%s\", \"depth\": %d, \"group\": \"%s\"}", idv, lbl, d, g
          }
          END { print "\n  ]," }
        ' "$modules_dot" > "$json_specific.tmp"
        awk 'BEGIN{ print "  \"edges\": ["; first=1 }
             /"[^"]+"[[:space:]]*->[[:space:]]*"[^"]+"/ {
               line=$0
               match(line,/"[^"]+"/); a=substr(line,RSTART+1,RLENGTH-2)
               rest=substr(line,RSTART+RLENGTH)
               match(rest,/"[^"]+"/); b=substr(rest,RSTART+1,RLENGTH-2)
               lbl=""; if (match(line,/label=\"[^\"]*\"/)) { lbl=substr(line,RSTART+7,RLENGTH-8) }
               if (!first) printf ",\n"; first=0
               printf "    {\"source\": \"%s\", \"target\": \"%s\", \"label\": \"%s\"}", a, b, lbl
             }
             END{ print "\n  ]\n}" }' "$modules_dot" >> "$json_specific.tmp"
        mv "$json_specific.tmp" "$json_specific" && cp "$json_specific" "$json_latest" || true
        echo "[modules] Wrote $modules_dot (colored + orphans panel) and $json_specific (+ graph_focus.json)"
        if command -v dot >/dev/null 2>&1; then
          if dot -Tsvg "$modules_dot" -o "$modules_svg"; then
            echo "[modules] Wrote $modules_svg"
          else
            echo "[modules] dot rendering failed (filtered). See $OUT_DIR/modules_lib.err"
          fi
        else
          echo "[modules] Graphviz 'dot' not found; skipped SVG rendering"
        fi
        set -e
        echo "[done] Outputs in: $OUT_DIR"
        exit 0
      else
  echo "[modules] No nodes matched segment '$last_segment' in module graph." | (tee -a "$OUT_DIR/modules_lib.err" >/dev/null || true)
        echo "[modules] Tip: check $struct_file for module names or try --bin <name>."
  # Ensure we do not pass an invalid --focus-on to the generic run
  FOCUS_ON=""
        # Fall through to generic (unfiltered) graph emission below
      fi
    else
      echo "[modules] Could not build probe graph to filter by segment." | (tee -a "$OUT_DIR/modules_lib.err" >/dev/null || true)
      # Fall through to generic (unfiltered) graph emission below
    fi
  fi

  # Build flags and output names (post-mapping)
  cm_flags=(dependencies --all-features --no-externs --no-fns --no-traits --no-types --no-uses)
  if [[ -n "$BIN_NAME" ]]; then
    cm_flags+=(--bin "$BIN_NAME")
    target_suffix="bin_${BIN_NAME}"
  else
    cm_flags+=(--lib)
    target_suffix="lib"
  fi
  if [[ -n "$FOCUS_ON" ]]; then
    cm_flags+=(--focus-on "$FOCUS_ON")
  fi
  if [[ -n "$MAX_DEPTH" ]]; then
    cm_flags+=(--max-depth "$MAX_DEPTH")
  fi
  out_suffix=""
  if [[ -n "$FOCUS_ON" ]]; then
    slug=$(echo "$FOCUS_ON" | tr ':' '_' | tr -cd '[:alnum:]_')
    out_suffix="${out_suffix}_focus_${slug}"
  fi
  if [[ -n "$MAX_DEPTH" ]]; then
    out_suffix="${out_suffix}_d${MAX_DEPTH}"
  fi
  modules_dot="$OUT_DIR/modules_${target_suffix}${out_suffix}.dot"
  modules_svg="$OUT_DIR/modules_${target_suffix}${out_suffix}.svg"
  modules_err="$OUT_DIR/modules_${target_suffix}${out_suffix}.err"

  if [[ -n "$PKG_NAME" ]]; then
    (cd "$SRC_TAURI_DIR" && cargo modules "${cm_flags[@]}" -p "$PKG_NAME" > "$modules_dot" 2>"$modules_err")
    status=$?
  else
    status=1
  fi
  if [[ $status -ne 0 ]]; then
    echo "[modules] Fallback: running without -p"
    (cd "$SRC_TAURI_DIR" && cargo modules "${cm_flags[@]}" > "$modules_dot" 2>"$modules_err")
    status=$?
  fi
  if [[ $status -ne 0 ]]; then
  echo "[modules] Second fallback: run with --manifest-path at workspace root"
    (cd "$ROOT_DIR" && cargo modules "${cm_flags[@]}" --manifest-path "$SRC_TAURI_DIR/Cargo.toml" > "$modules_dot" 2>>"$modules_err")
    status=$?
  fi
  set -e
  if [[ $status -eq 0 ]]; then
    echo "[modules] Wrote $modules_dot"
    if command -v dot >/dev/null 2>&1; then
      if dot -Tsvg "$modules_dot" -o "$modules_svg"; then
        echo "[modules] Wrote $modules_svg"
      else
        echo "[modules] dot rendering failed (modules). See $modules_err"
      fi
    else
      echo "[modules] Graphviz 'dot' not found; skipped SVG rendering"
    fi
   # Emit JSON for interactive viewer (generic/unfiltered case)
    json_latest="$OUT_DIR/graph_focus.json"
    json_from_generic="$OUT_DIR/modules_${target_suffix}${out_suffix}.json"
   # Compute a base/root that prefers module nodes (label contains mod|) with maximum outdegree; fallback to lexicographically smallest
    awk '
      /^[[:space:]]*"[^"]+"[[:space:]]*\[/ {
        match($0,/"[^"]+"/); name=substr($0,RSTART+1,RLENGTH-2); nodes[name]=1; labels[name]=name;
        ismod[name]=0; if (match($0,/label=\"[^\"]*\"/)) { labels[name]=substr($0,RSTART+7,RLENGTH-8); if (labels[name] ~ /(^|\|)mod\|/) ismod[name]=1 }
      }
      /"[^\"]+"[[:space:]]*->[[:space:]]*"[^\"]+"/ {
            line=$0; match(line,/"[^"]+"/); a=substr(line,RSTART+1,RLENGTH-2); rest=substr(line,RSTART+RLENGTH); match(rest,/"[^"]+"/); b=substr(rest,RSTART+1,RLENGTH-2); out[a]++;
            edges[++m]=a "\t" b
         }
         END {
        base=""; maxo=-1;
        # prefer module nodes
        for (n in nodes) { if (ismod[n]) { o=(n in out? out[n]:0); if (o>maxo) { maxo=o; base=n } } }
        if (base=="") { for (n in nodes) { o=(n in out? out[n]:0); if (o>maxo) { maxo=o; base=n } } }
        if (base=="") { for (n in nodes) { base=n; break } }
           # Start JSON
           print "{\n  \"base\": \"" base "\",\n  \"nodes\": ["; first=1
           for (n in nodes) {
             lbl=labels[n]; gsub(/\\l/, "\n", lbl); gsub(/\\/, "\\\\", lbl); gsub(/\"/, "\\\"", lbl); gsub(/\t/, "\\t", lbl); gsub(/\r/, "\\r", lbl); gsub(/\n/, "\\n", lbl);
             # depth relative to base (approx)
             d=0; if (index(n, base "::")==1) { tail=substr(n, length(base)+3); if (tail!="") { split(tail, tmp, /::/); d=length(tmp) } }
             idv=n; gsub(/\\/, "\\\\", idv); gsub(/\"/, "\\\"", idv);
             if (!first) printf ",\n"; first=0;
             printf "    {\"id\": \"%s\", \"label\": \"%s\", \"depth\": %d, \"group\": \"\"}", idv, lbl, d
           }
           print "\n  ],\n  \"edges\": ["; first=1
           for (i=1;i<=m;i++) {
             split(edges[i], ab, /\t/); a=ab[1]; b=ab[2];
             if (!first) printf ",\n"; first=0;
             printf "    {\"source\": \"%s\", \"target\": \"%s\", \"label\": \"\"}", a, b
           }
           print "\n  ]\n}"
         }
        ' "$modules_dot" > "$json_from_generic.tmp" && mv "$json_from_generic.tmp" "$json_from_generic" && cp "$json_from_generic" "$json_latest" || true
  else
    echo "[modules] cargo modules failed. See $modules_err"
  fi
else
  echo "[modules] cargo-modules not installed. Run scripts/install_dep_vis_tools.sh"
fi

echo "[done] Outputs in: $OUT_DIR"
