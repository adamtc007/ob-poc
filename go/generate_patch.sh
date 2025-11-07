#!/bin/bash

# generate_patch.sh - Generate my_changes.patch from current git diff
# Usage: ./generate_patch.sh

set -e  # Exit on any error

echo "📝 Generating my_changes.patch from current git diff..."
echo ""

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "❌ Error: Not in a git repository"
    exit 1
fi

# Check if there are any changes to capture
if git diff --quiet && git diff --cached --quiet; then
    echo "ℹ️  No changes detected in working directory or staging area"
    echo "   Current git status:"
    git status --porcelain
    echo ""
    echo "   If you have untracked files you want to include:"
    echo "   1. Add them: git add <files>"
    echo "   2. Then run this script again"
    exit 0
fi

# Remove existing patch file if it exists
if [ -f "my_changes.patch" ]; then
    echo "🗑️  Removing existing my_changes.patch"
    rm my_changes.patch
fi

# Generate patch file including both staged and unstaged changes
echo "📦 Generating patch file..."
echo "   Capturing both staged and unstaged changes"

# Combine both staged and unstaged changes into one patch
git diff HEAD > my_changes.patch

# Check if patch file was created and has content
if [ ! -f "my_changes.patch" ]; then
    echo "❌ Error: Failed to create my_changes.patch"
    exit 1
fi

if [ ! -s "my_changes.patch" ]; then
    echo "⚠️  Warning: my_changes.patch is empty"
    rm my_changes.patch
    echo "   This usually means all changes are already committed"
    echo "   Current git status:"
    git status --short
    exit 0
fi

# Show patch file info
echo "   ✅ my_changes.patch created successfully"
echo ""
echo "📊 Patch file statistics:"
echo "   Size: $(wc -c < my_changes.patch) bytes"
echo "   Lines: $(wc -l < my_changes.patch) lines"
echo ""

# Show what files are included in the patch
echo "📁 Files included in patch:"
grep '^diff --git' my_changes.patch | sed 's/diff --git a\//   - /' | sed 's/ b\/.*//'

echo ""
echo "🎉 my_changes.patch is ready for upload!"
echo ""
echo "Next steps:"
echo "1. Upload my_changes.patch to Claude/Gemini"
echo "2. Wait for gemini_response.patch"
echo "3. Apply it with: ./apply_patch.sh gemini_response.patch"