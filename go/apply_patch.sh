#!/bin/bash

# apply_patch.sh - Apply a patch file (typically gemini_response.patch)
# Usage: ./apply_patch.sh <patch_file>

set -e  # Exit on any error

# Check if patch file argument is provided
if [ $# -eq 0 ]; then
    echo "❌ Error: No patch file specified"
    echo ""
    echo "Usage: ./apply_patch.sh <patch_file>"
    echo "Example: ./apply_patch.sh gemini_response.patch"
    exit 1
fi

PATCH_FILE="$1"

echo "🔧 Applying patch file: $PATCH_FILE"
echo ""

# Check if we're in a git repository
if ! git rev-parse --git-dir > /dev/null 2>&1; then
    echo "❌ Error: Not in a git repository"
    exit 1
fi

# Check if patch file exists
if [ ! -f "$PATCH_FILE" ]; then
    echo "❌ Error: Patch file '$PATCH_FILE' not found"
    exit 1
fi

# Check if patch file has content
if [ ! -s "$PATCH_FILE" ]; then
    echo "❌ Error: Patch file '$PATCH_FILE' is empty"
    exit 1
fi

# Show patch file info
echo "📊 Patch file statistics:"
echo "   Size: $(wc -c < "$PATCH_FILE") bytes"
echo "   Lines: $(wc -l < "$PATCH_FILE") lines"
echo ""

# Show what files will be modified
echo "📁 Files to be modified:"
if grep -q '^diff --git' "$PATCH_FILE"; then
    grep '^diff --git' "$PATCH_FILE" | sed 's/diff --git a\//   - /' | sed 's/ b\/.*//'
else
    echo "   (Unable to detect files - patch format may be different)"
fi
echo ""

# Check for uncommitted changes
if ! git diff --quiet || ! git diff --cached --quiet; then
    echo "⚠️  Warning: You have uncommitted changes in your working directory"
    echo ""
    git status --short
    echo ""
    read -p "Do you want to continue applying the patch? (y/N): " -n 1 -r
    echo ""
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        echo "🚫 Patch application cancelled"
        echo ""
        echo "Consider:"
        echo "1. Commit your current changes: git add . && git commit -m 'WIP: before applying patch'"
        echo "2. Or stash them: git stash"
        echo "3. Then run this script again"
        exit 1
    fi
    echo ""
fi

# Try to apply the patch
echo "🔄 Applying patch..."

# First, try a dry run to check if patch can be applied cleanly
if git apply --check "$PATCH_FILE" 2>/dev/null; then
    echo "   ✅ Patch validation successful"

    # Apply the patch
    if git apply "$PATCH_FILE"; then
        echo "   ✅ Patch applied successfully"
        echo ""

        # Show what changed
        echo "📈 Changes applied:"
        git diff --stat
        echo ""

        echo "🎉 Patch application complete!"
        echo ""
        echo "Next steps:"
        echo "1. Review the changes: git diff"
        echo "2. Test the changes: make test"
        echo "3. Commit if satisfied: git add . && git commit -m 'Apply gemini changes'"
        echo "4. Or generate new patch: ./generate_patch.sh"

    else
        echo "❌ Error: Failed to apply patch"
        exit 1
    fi

else
    echo "⚠️  Patch validation failed - attempting to apply with conflict detection..."

    # Try to apply with 3-way merge
    if git apply --3way "$PATCH_FILE" 2>/dev/null; then
        echo "   ✅ Patch applied with 3-way merge"
        echo ""
        echo "📈 Changes applied:"
        git diff --stat
        echo ""
        echo "🎉 Patch application complete!"

    else
        echo "❌ Error: Patch could not be applied"
        echo ""
        echo "This usually means:"
        echo "1. The patch was created from a different base commit"
        echo "2. There are conflicting changes in your working directory"
        echo "3. Files in the patch have been moved or deleted"
        echo ""
        echo "Troubleshooting:"
        echo "1. Check git status: git status"
        echo "2. Review the patch file: cat $PATCH_FILE"
        echo "3. Try manual application or resolve conflicts"
        exit 1
    fi
fi