#!/bin/sh
# usage: caps.sh script.lic [more...] -- print header (first 40 lines) and capture lines
L=E:/Cena/reference/lich_repo_mirror/lib
for s in "$@"; do
  echo "=================== $s ($(wc -l < $L/$s) lines)"
  sed -n '1,30p' $L/$s | grep -iE "author|version|date|license|purpose|^ *#|=begin|description|tags|game" | head -15
  echo "--- captures"
  grep -nE "=~ */|waitforre|matchtimeout|matchwait|matchfind|DownstreamHook|UpstreamHook|when +/|Regexp.new|%r\{|\.match\(|scan\(/|waitfor |match\(|XMLData\.|GameObj\.|Spell\[|Effects::|Char\.|Skills\.|Stats\.|Bounty|Wounds|Scars|checkpcs|checknpcs|pushStream|<[a-zA-Z]+ id=" $L/$s | cut -c1-240 | head -${MAXL:-80}
done
