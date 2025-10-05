# 3D Models Directory

## Required Files for Premium Hero

### Primary 3D Model
- **File**: `research_scene_draco.glb`
- **Description**: Draco-compressed GLB model of a research scene
- **Size**: Under 2MB recommended
- **Format**: GLB with Draco compression
- **Textures**: KTX2 format for optimal performance

### Fallback Poster Image
- **File**: `model-poster.png` (in `/images/` directory)
- **Dimensions**: 1600×1200 pixels
- **Description**: High-quality poster image for 3D model fallback
- **Optimization**: WebP format recommended for better compression

## Model Specifications

### Aesthetic Requirements
- Photoreal or handcrafted research-themed 3D scene
- Examples: stylized paper stack, floating lab notebook with holographic annotations, elegant DNA helix abstract
- Professional finish (not AI-generated looking)

### Performance Requirements
- Draco compression for geometry
- KTX2 textures for optimal loading
- LODs (Level of Detail) for complex models
- Lazy loading with poster fallback

### Interaction Features
- Auto-rotate at 0.02 rotation-per-second
- Pause rotation on hover/touch
- Camera controls enabled
- Shadow intensity: 1
- AR support enabled

## Asset Sources (Recommended)

### Premium Models
- **Sketchfab Premium**: Professional research-themed models
- **TurboSquid**: High-quality 3D assets with web licenses
- **Poly Haven**: Free high-quality models

### Search Terms
- "photoreal still life"
- "studio product mockup"
- "stylized paper stack"
- "lab notebook"
- "research equipment"

## Implementation Notes

The model-viewer component will automatically:
1. Show poster image immediately
2. Load 3D model in background
3. Switch to 3D model when ready
4. Provide fallback for unsupported browsers

## Current Status

⚠️ **Placeholder Required**: Add actual 3D model files to complete implementation.

The current implementation shows a beautiful fallback design while the 3D model loads.
