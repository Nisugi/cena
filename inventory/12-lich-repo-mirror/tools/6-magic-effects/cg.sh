#!/bin/sh
# usage: cg.sh "fragment" ... ; greps Cena crates (rs, tsv) and lich-5 lib for each fragment
for f in "$@"; do
  c=$(grep -rlF --include=*.rs --include=*.tsv --include=*.toml -- "$f" E:/Cena/crates 2>/dev/null | head -5 | tr '\n' ' ')
  l=$(grep -rlF -- "$f" E:/Cena/reference/lich-5/lib 2>/dev/null | head -3 | sed 's#E:/Cena/reference/lich-5/##' | tr '\n' ' ')
  printf '%s\n   CENA: %s\n   LICH: %s\n' "$f" "${c:-none}" "${l:-none}"
done
