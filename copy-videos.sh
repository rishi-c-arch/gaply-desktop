#!/bin/bash

# Script to copy videos from Desktop to React public folder
# Run this script from the gaply-react directory

echo "Copying videos from Desktop to public/videos/..."

# Create videos directory if it doesn't exist
mkdir -p public/videos

# Copy videos (you may need to adjust the exact filenames)
echo "Please manually copy these files from your Desktop to public/videos/:"
echo "1. Screen Recording 2025-10-25 at 12.45.06 AM.mov"
echo "2. Screen Recording 2025-10-25 at 12.47.07 AM.mov" 
echo "3. Screen Recording 2025-10-25 at 12.48.16 AM.mov"
echo ""
echo "You can do this by:"
echo "1. Opening Finder"
echo "2. Navigate to your Desktop"
echo "3. Select the three video files"
echo "4. Copy them (Cmd+C)"
echo "5. Navigate to /Users/rishi/Desktop/GAPLY/gaply-react/public/videos/"
echo "6. Paste them (Cmd+V)"
echo ""
echo "Or use Terminal:"
echo "cp ~/Desktop/Screen\\ Recording\\ 2025-10-25\\ at\\ 12.45.06\\ AM.mov public/videos/"
echo "cp ~/Desktop/Screen\\ Recording\\ 2025-10-25\\ at\\ 12.47.07\\ AM.mov public/videos/"
echo "cp ~/Desktop/Screen\\ Recording\\ 2025-10-25\\ at\\ 12.48.16\\ AM.mov public/videos/"
