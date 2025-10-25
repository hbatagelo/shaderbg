#!/bin/bash
# Generate man page from markdown

if [ $# -ne 1 ]; then
    echo "Usage: $0 <source.md>"
    exit 1
fi

SOURCE_MD="$1"
MANPAGE="${SOURCE_MD%.md}"

if [ ! -f "$SOURCE_MD" ]; then
    echo "Error: Source file $SOURCE_MD not found"
    exit 1
fi

if ! command -v pandoc &> /dev/null; then
    echo "Error: pandoc is not installed"
    exit 1
fi

pandoc "$SOURCE_MD" -s -t man -o "$MANPAGE"

if [ $? -ne 0 ]; then
    echo "Error: pandoc failed to generate man page"
    exit 1
fi

sed -i '
s/\\f\[C\]/\\f[B]/g
s/\\f\[CB\]/\\f[B]/g
s/\\f\[CI\]/\\f[I]/g
s/\\f\[CR\]/\\f[R]/g
s/©/\\(co/g
' "$MANPAGE"

echo "Generated $MANPAGE"

if groff -man -T ascii "$MANPAGE" > /dev/null; then
    echo "✓ Man page generated successfully with no groff warnings"
else
    echo "⚠ Warning: groff reported issues with the generated man page"
    groff -man -T ascii "$MANPAGE" > /dev/null
fi