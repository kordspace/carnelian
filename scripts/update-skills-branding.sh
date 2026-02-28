#!/bin/bash
# Update all skills documentation to use CARNELIAN branding instead of OpenClaw/Thummim

set -e

SKILLS_DIR="skills/core-registry"
BACKUP_DIR=".skills-backup-$(date +%Y%m%d-%H%M%S)"

echo "🔥 CARNELIAN Skills Branding Update"
echo "===================================="
echo ""

# Create backup
echo "Creating backup in $BACKUP_DIR..."
cp -r "$SKILLS_DIR" "$BACKUP_DIR"

# Count files to update
SKILL_MD_COUNT=$(find "$SKILLS_DIR" -name "SKILL.md" | wc -l)
SKILL_JSON_COUNT=$(find "$SKILLS_DIR" -name "skill.json" | wc -l)

echo "Found $SKILL_MD_COUNT SKILL.md files"
echo "Found $SKILL_JSON_COUNT skill.json files"
echo ""

# Update SKILL.md files
echo "Updating SKILL.md files..."
find "$SKILLS_DIR" -name "SKILL.md" -type f | while read -r file; do
    # Replace OpenClaw references
    sed -i 's/OpenClaw/CARNELIAN/g' "$file"
    sed -i 's/openclaw/carnelian/g' "$file"
    
    # Replace Thummim references
    sed -i 's/Thummim/CARNELIAN/g' "$file"
    sed -i 's/thummim/carnelian/g' "$file"
    
    # Update repository references
    sed -i 's|github\.com/kordspace/openclaw|github.com/kordspace/carnelian|g' "$file"
    sed -i 's|github\.com/kordspace/thummim|github.com/kordspace/carnelian|g' "$file"
    
    echo "  ✓ Updated: $file"
done

# Update skill.json files
echo ""
echo "Updating skill.json files..."
find "$SKILLS_DIR" -name "skill.json" -type f | while read -r file; do
    # Replace OpenClaw references in JSON
    sed -i 's/"openclaw"/"carnelian"/g' "$file"
    sed -i 's/"OpenClaw"/"CARNELIAN"/g' "$file"
    
    # Replace Thummim references in JSON
    sed -i 's/"thummim"/"carnelian"/g' "$file"
    sed -i 's/"Thummim"/"CARNELIAN"/g' "$file"
    
    echo "  ✓ Updated: $file"
done

echo ""
echo "✅ Skills branding update complete!"
echo ""
echo "Backup saved to: $BACKUP_DIR"
echo "Review changes and delete backup if satisfied."
