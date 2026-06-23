from PIL import Image, ImageDraw

def create_brain_icon(size):
    img = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    cx = cy = size // 2
    r = size * 0.45
    
    # Glow circle
    for i in range(5, 0, -1):
        alpha = int(15 / i)
        draw.ellipse([cx-r-i*3, cy-r-i*3, cx+r+i*3, cy+r+i*3], 
                     outline=(63, 185, 80, alpha), width=1)
    
    # Brain hemispheres (two arcs)
    # Left hemisphere
    draw.arc([cx-r, cy-r, cx, cy+r], 180, 0, fill=(63, 185, 80), width=max(3, size//40))
    # Right hemisphere
    draw.arc([cx, cy-r, cx+r, cy+r], 180, 0, fill=(63, 185, 80), width=max(3, size//40))
    
    # Top fold
    fold_y = int(cy - r * 0.4)
    draw.arc([cx-int(r*0.6), fold_y-5, cx+int(r*0.6), fold_y+15], 
             0, 180, fill=(63, 185, 80, 180), width=max(2, size//50))
    
    # Neural dots
    dots = [
        (cx - r*0.4, cy - r*0.2),
        (cx - r*0.2, cy + r*0.1),
        (cx, cy - r*0.3),
        (cx + r*0.2, cy + r*0.05),
        (cx + r*0.4, cy - r*0.15),
        (cx - r*0.25, cy + r*0.3),
        (cx + r*0.25, cy + r*0.25),
    ]
    dot_r = max(2, size//50)
    for dx, dy in dots:
        draw.ellipse([dx-dot_r, dy-dot_r, dx+dot_r, dy+dot_r], fill=(0, 212, 255))
    
    # Neural connections
    connections = [(0,1), (1,2), (2,3), (3,4), (5,1), (6,3)]
    for i, j in connections:
        x1, y1 = dots[i]
        x2, y2 = dots[j]
        draw.line([x1, y1, x2, y2], fill=(0, 212, 255, 120), width=max(1, size//80))
    
    # Brain stem
    stem_top = int(cy + r * 0.25)
    stem_bot = int(cy + r * 0.55)
    draw.line([cx-3, stem_top, cx-3, stem_bot], fill=(63, 185, 80, 150), width=max(2, size//40))
    draw.line([cx+3, stem_top, cx+3, stem_bot], fill=(63, 185, 80, 150), width=max(2, size//40))
    
    return img

# Generate icons
for name, size in [("icon_256.png", 256), ("icon_64.png", 64), ("icon_32.png", 32)]:
    img = create_brain_icon(size)
    img.save(f"electron/{name}")
    print(f"{name}: {__import__('os').path.getsize(f'electron/{name}')} bytes")

# Create ICO
img256 = Image.open("electron/icon_256.png")
img64 = Image.open("electron/icon_64.png")
img32 = Image.open("electron/icon_32.png")
img16 = create_brain_icon(16)

img256.save("electron/icon.ico", format="ICO", sizes=[(256,256), (64,64), (32,32), (16,16)])
print(f"icon.ico: {__import__('os').path.getsize('electron/icon.ico')} bytes")
print("OK")
