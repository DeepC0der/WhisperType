import os
import colorsys
from PIL import Image, ImageDraw, ImageFont

def shift_hue_to_ice_blue(img):
    img = img.convert("RGBA")
    pixels = img.load()
    w, h = img.size
    
    out = Image.new("RGBA", (w, h))
    out_pixels = out.load()
    
    for y in range(h):
        for x in range(w):
            r, g, b, a = pixels[x, y]
            if a == 0:
                out_pixels[x, y] = (r, g, b, 0)
                continue
                
            h_val, s_val, v_val = colorsys.rgb_to_hsv(r / 255.0, g / 255.0, b / 255.0)
            h_deg = h_val * 360.0
            
            if 115.0 <= h_deg <= 190.0:
                t = (h_deg - 115.0) / (190.0 - 115.0)
                new_h_deg = 194.0 + t * 10.0 # 194° to 204° (Sky-Cyan)
                new_h = (new_h_deg % 360.0) / 360.0
                new_s = min(1.0, s_val * 1.05)
                new_v = min(1.0, v_val * 1.05)
                nr, ng, nb = colorsys.hsv_to_rgb(new_h, new_s, new_v)
                out_pixels[x, y] = (int(nr * 255.0), int(ng * 255.0), int(nb * 255.0), a)
            elif 95.0 <= h_deg < 115.0:
                blend = (h_deg - 95.0) / 20.0
                target_h_deg = 194.0
                new_h_deg = (h_deg * (1.0 - blend) + target_h_deg * blend)
                new_h = (new_h_deg % 360.0) / 360.0
                nr, ng, nb = colorsys.hsv_to_rgb(new_h, s_val, v_val)
                out_pixels[x, y] = (int(nr * 255.0), int(ng * 255.0), int(nb * 255.0), a)
            elif 190.0 < h_deg <= 220.0:
                nr, ng, nb = colorsys.hsv_to_rgb(h_val, min(1.0, s_val * 1.05), min(1.0, v_val * 1.05))
                out_pixels[x, y] = (int(nr * 255.0), int(ng * 255.0), int(nb * 255.0), a)
            else:
                out_pixels[x, y] = (r, g, b, a)
                
    return out

def get_font(size):
    font_paths = [
        "C:/Windows/Fonts/segoeuib.ttf",
        "C:/Windows/Fonts/arialbd.ttf",
        "C:/Windows/Fonts/tahomabd.ttf"
    ]
    for p in font_paths:
        if os.path.exists(p):
            try:
                return ImageFont.truetype(p, size)
            except Exception:
                pass
    return ImageFont.load_default()

def draw_pill_badge_at_size(base_img):
    w, h = base_img.size
    result = base_img.copy()
    draw = ImageDraw.Draw(result)
    
    if w >= 256:
        scale = w / 512.0
        pill_w = int(216 * scale)
        pill_h = int(92 * scale)
        pad_r = int(32 * scale)
        pad_b = int(32 * scale)
        font_size = int(54 * scale)
        border_w = max(2, int(3 * scale))
        shadow_w = max(2, int(4 * scale))
    elif w >= 128:
        pill_w = 68
        pill_h = 30
        pad_r = 8
        pad_b = 8
        font_size = 18
        border_w = 2
        shadow_w = 2
    elif w >= 48:
        pill_w = 34
        pill_h = 16
        pad_r = 2
        pad_b = 2
        font_size = 11
        border_w = 1
        shadow_w = 1
    elif w >= 32:
        pill_w = 26
        pill_h = 13
        pad_r = 1
        pad_b = 1
        font_size = 9
        border_w = 1
        shadow_w = 1
    elif w >= 24:
        pill_w = 20
        pill_h = 10
        pad_r = 1
        pad_b = 1
        font_size = 7
        border_w = 1
        shadow_w = 1
    else: # 16x16
        pill_w = 15
        pill_h = 8
        pad_r = 0
        pad_b = 0
        font_size = 6
        border_w = 1
        shadow_w = 0

    x2 = w - pad_r
    y2 = h - pad_b
    x1 = x2 - pill_w
    y1 = y2 - pill_h
    radius = pill_h // 2
    
    # Shadow outline
    if shadow_w > 0:
        draw.rounded_rectangle(
            [x1 - shadow_w, y1 - shadow_w, x2 + shadow_w, y2 + shadow_w],
            radius=radius + shadow_w,
            fill=(2, 10, 20, 240)
        )
        
    # Vibrant Sky-Blue Pill (#0284c7)
    draw.rounded_rectangle(
        [x1, y1, x2, y2],
        radius=radius,
        fill=(2, 132, 199, 255)
    )
    
    # Crisp Cyan inner border (#38bdf8)
    if border_w > 0 and (x2 - 2*border_w > x1) and (y2 - 2*border_w > y1):
        draw.rounded_rectangle(
            [x1 + 1, y1 + 1, x2 - 1, y2 - 1],
            radius=max(1, radius - 1),
            outline=(56, 189, 248, 255),
            width=border_w
        )
        
    font = get_font(font_size)
    text = "LITE"
    tb = draw.textbbox((0, 0), text, font=font)
    tw = tb[2] - tb[0]
    th = tb[3] - tb[1]
    
    tx = x1 + (pill_w - tw) // 2 - tb[0]
    ty = y1 + (pill_h - th) // 2 - tb[1] - (1 if w >= 48 else 0)
    
    draw.text((tx, ty), text, font=font, fill=(255, 255, 255, 255))
    return result

def main():
    src_icon_path = 'D:/ProjectsPersonal/Murmur/src-tauri/icons/icon.png'
    dest_icons_dir = 'D:/ProjectsPersonal/Murmur/whispertype-lite/src-tauri/icons'
    dest_public_logo = 'D:/ProjectsPersonal/Murmur/whispertype-lite/public/logo.png'
    
    os.makedirs(dest_icons_dir, exist_ok=True)
    os.makedirs(os.path.dirname(dest_public_logo), exist_ok=True)
    
    orig = Image.open(src_icon_path)
    shifted_base = shift_hue_to_ice_blue(orig)
    
    # 1. Base 512x512
    icon_512 = draw_pill_badge_at_size(shifted_base.resize((512, 512), Image.Resampling.LANCZOS))
    icon_512.save(os.path.join(dest_icons_dir, 'icon.png'))
    
    # 2. Specific sizes
    sizes = {
        '128x128@2x.png': 256,
        '128x128.png': 128,
        '32x32.png': 32,
        'Square30x30Logo.png': 30,
        'Square44x44Logo.png': 44,
        'Square71x71Logo.png': 71,
        'Square89x89Logo.png': 89,
        'Square107x107Logo.png': 107,
        'Square142x142Logo.png': 142,
        'Square150x150Logo.png': 150,
        'Square284x284Logo.png': 284,
        'Square310x310Logo.png': 310,
        'StoreLogo.png': 50,
    }
    
    for filename, sz in sizes.items():
        img_sz = shifted_base.resize((sz, sz), Image.Resampling.LANCZOS)
        badged = draw_pill_badge_at_size(img_sz)
        badged.save(os.path.join(dest_icons_dir, filename))
        
    # 3. Public logo.png (128x128)
    logo_128 = draw_pill_badge_at_size(shifted_base.resize((128, 128), Image.Resampling.LANCZOS))
    logo_128.save(dest_public_logo)
    
    # 4. Multi-size icon.ico
    # Windows icon sizes: 16, 24, 32, 48, 64, 128, 256
    ico_sizes = [16, 24, 32, 48, 64, 128, 256]
    ico_images = []
    for sz in ico_sizes:
        base_sz = shifted_base.resize((sz, sz), Image.Resampling.LANCZOS)
        ico_images.append(draw_pill_badge_at_size(base_sz))
        
    ico_path = os.path.join(dest_icons_dir, 'icon.ico')
    # Save first image (256x256) and append the rest
    ico_images[-1].save(
        ico_path,
        format='ICO',
        sizes=[(img.width, img.height) for img in ico_images]
    )
    print("Successfully generated all WhisperType Lite icons!")

if __name__ == "__main__":
    main()
