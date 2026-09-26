#!/usr/bin/env bash
# Usage: probe.sh "fragment1" "fragment2" ...
# For each fragment, prints up to 4 hits across Cena crates (rs, tsv), case-insensitive fixed string.
cd /e/Cena/crates || exit 1
for f in "$@"; do
  n=$(grep -rni --include=*.rs --include=*.tsv -e "$f" . 2>/dev/null | wc -l)
  echo "== [$f] hits=$n"
  grep -rni --include=*.rs --include=*.tsv -e "$f" . 2>/dev/null | head -4 | cut -c1-200
done
