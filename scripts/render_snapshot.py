#!/usr/bin/env python3
"""Render a Ratatui cell capture to PNG for development review (requires Pillow)."""
import argparse
import json
from pathlib import Path
from PIL import Image, ImageDraw, ImageFont

parser = argparse.ArgumentParser()
parser.add_argument("source", type=Path)
parser.add_argument("destination", type=Path)
parser.add_argument("--font", type=Path, default=Path("/System/Library/Fonts/STHeiti Light.ttc"))
args = parser.parse_args()
capture = json.loads(args.source.read_text())
font = ImageFont.truetype(str(args.font), 16)
cell_width, cell_height, margin = 10, 22, 18
image = Image.new("RGB", (capture["width"] * cell_width + margin * 2, capture["height"] * cell_height + margin * 2), "#0c141d")
draw = ImageDraw.Draw(image)
for index, cell in enumerate(capture["cells"]):
    x = margin + index % capture["width"] * cell_width
    y = margin + index // capture["width"] * cell_height
    draw.rectangle((x, y, x + cell_width, y + cell_height), fill=cell["bg"])
for index, cell in enumerate(capture["cells"]):
    if not cell["symbol"].strip():
        continue
    x = margin + index % capture["width"] * cell_width
    y = margin + index // capture["width"] * cell_height
    draw.text((x, y + 2), cell["symbol"], font=font, fill=cell["fg"])
args.destination.parent.mkdir(parents=True, exist_ok=True)
image.save(args.destination)
