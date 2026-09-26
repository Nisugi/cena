#!/bin/sh
# cg.sh "phrase" ... : for each fixed-string phrase, list Cena source/data files containing it
# (crates/*/src, crates/*/data; tests excluded unless CG_TESTS=1)
for s in "$@"; do
  if [ -n "$CG_TESTS" ]; then
    hits=$(grep -rlF --include=*.rs --include=*.tsv -e "$s" E:/Cena/crates 2>/dev/null | sed 's#E:/Cena/##' | tr '\n' ' ')
  else
    hits=$(grep -rlF --include=*.rs --include=*.tsv -e "$s" E:/Cena/crates/*/src E:/Cena/crates/*/data 2>/dev/null | sed 's#E:/Cena/##' | tr '\n' ' ')
  fi
  printf '%-40s | %s\n' "$s" "${hits:-NONE}"
done
