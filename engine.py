"""Conservative, configurable comic restoration and archive handling."""
import io
import re
import zipfile
from pathlib import PurePosixPath
import xml.etree.ElementTree as ET

import numpy as np
from PIL import Image, ImageFilter, ImageOps

PRESETS = {
    'digital': {},
    'scan': {'deskew': True, 'crop': True, 'paper': 0.35},
    'manga': {'deskew': True, 'crop': True, 'paper': 0.25, 'grayscale': True},
}
EXTENSIONS = {'.jpg', '.jpeg', '.png', '.webp', '.bmp', '.tif', '.tiff'}
MAX_BYTES = 512 * 1024 * 1024
Image.MAX_IMAGE_PIXELS = 32_000_000


def natural_key(name):
    return [int(part) if part.isdigit() else part.lower() for part in re.split(r'(\d+)', name)]


def read_archive(data, filename):
    if len(data) > MAX_BYTES:
        raise ValueError('Archief groter dan 512 MB.')
    if filename.lower().endswith('.cbr'):
        import rarfile
        archive = rarfile.RarFile(io.BytesIO(data))
    else:
        archive = zipfile.ZipFile(io.BytesIO(data))
    pages, metadata, warnings = [], None, []
    with archive:
        entries = archive.infolist()
        if len(entries) > 3000 or sum(e.file_size for e in entries) > MAX_BYTES:
            raise ValueError('Archief bevat te veel bestanden of uitgepakte gegevens.')
        for entry in sorted(entries, key=lambda e: natural_key(e.filename)):
            name = entry.filename
            if PurePosixPath(name).name.lower() == 'comicinfo.xml':
                metadata = archive.read(entry)
            elif PurePosixPath(name).suffix.lower() in EXTENSIONS:
                try:
                    with Image.open(io.BytesIO(archive.read(entry))) as source:
                        source.load()
                        if source.width * source.height > 32_000_000:
                            raise ValueError('Pagina groter dan 32 megapixels')
                        pages.append((name, ImageOps.exif_transpose(source).convert('RGB')))
                except Exception as error:
                    warnings.append(f'{name}: onleesbaar ({error})')
    if not pages:
        raise ValueError('Geen leesbare pagina’s gevonden.')
    return pages, metadata, warnings


def inspect_pages(pages):
    report, hashes = [], []
    for name, image in pages:
        gray = np.asarray(image.convert('L').resize((9, 8)), dtype=float)
        fingerprint = (gray[:, 1:] > gray[:, :-1]).flatten()
        duplicate = next((pages[i][0] for i, old in enumerate(hashes)
                          if np.count_nonzero(old != fingerprint) <= 3), None)
        hashes.append(fingerprint)
        reasons = []
        if duplicate:
            reasons.append('Mogelijk dubbel met ' + duplicate)
        if image.width > image.height * 1.25:
            reasons.append('Mogelijke dubbele pagina')
        if min(image.size) < 700:
            reasons.append('Lage resolutie')
        report.append({'name': name, 'width': image.width, 'height': image.height,
                       'review': reasons})
    return report


def restore(image, options):
    result = image.copy()
    if options.get('deskew'):
        small = result.convert('L')
        small.thumbnail((600, 600))
        scores = []
        for angle in np.arange(-3, 3.01, 0.25):
            rotated = np.asarray(small.rotate(float(angle), fillcolor=255))
            ink = (rotated < 100).mean(axis=1)
            scores.append((float(np.var(ink)), float(angle)))
        baseline = scores[12][0]
        score, angle = max(scores)
        if score > baseline * 1.08 and abs(angle) < 3:
            result = result.rotate(angle, resample=Image.Resampling.BICUBIC,
                                   expand=True, fillcolor='white')
    if options.get('crop'):
        # Crop only uniformly light/dark borders, capped at 5% per side.
        array = np.asarray(result.convert('L'))
        rows = ((array > 245).mean(axis=1) > .995) | ((array < 12).mean(axis=1) > .995)
        columns = ((array > 245).mean(axis=0) > .995) | ((array < 12).mean(axis=0) > .995)
        def edge(flags, limit):
            count = 0
            for flag in flags[:limit]:
                if not flag:
                    break
                count += 1
            return max(0, count - 8)
        width, height = result.size
        result = result.crop((edge(columns, width // 20), edge(rows, height // 20),
                              width - edge(columns[::-1], width // 20),
                              height - edge(rows[::-1], height // 20)))
    paper = float(options.get('paper', 0))
    if paper:
        array = np.asarray(result, dtype=float)
        white = np.percentile(array.reshape(-1, 3), 95, axis=0)
        corrected = array * (255 / np.maximum(white, 180))
        result = Image.fromarray(np.uint8(np.clip(array * (1-paper) + corrected * paper, 0, 255)))
    shadow = float(options.get('shadow', 0))
    if shadow:
        background = result.resize((64, 64)).filter(ImageFilter.GaussianBlur(8)).resize(result.size)
        array = np.asarray(result, dtype=float)
        illumination = np.asarray(background, dtype=float)
        corrected = array * 230 / np.maximum(illumination, 120)
        result = Image.fromarray(np.uint8(np.clip(array*(1-shadow) + corrected*shadow, 0, 255)))
    bleed = float(options.get('bleed', 0))
    if bleed:
        array = np.asarray(result, dtype=float)
        light = np.min(array, axis=2) > 170
        array[light] += (255-array[light]) * bleed
        result = Image.fromarray(np.uint8(np.clip(array, 0, 255)))
    moire = float(options.get('moire', 0))
    if moire:
        result = Image.blend(result, result.filter(ImageFilter.MedianFilter(3)), moire)
    if options.get('grayscale'):
        result = ImageOps.grayscale(result).convert('RGB')
    if options.get('upscale'):
        if result.width * result.height > 16_000_000:
            raise ValueError('2× opschalen beperkt tot invoer van 16 megapixels.')
        if options.get('gpu'):
            import torch
            if not torch.cuda.is_available():
                raise ValueError('CUDA niet beschikbaar. Kies CPU of installeer PyTorch met CUDA.')
            tensor = torch.from_numpy(np.asarray(result).copy()).permute(2,0,1).unsqueeze(0).float().cuda()
            with torch.no_grad():
                output = torch.nn.functional.interpolate(tensor, scale_factor=2, mode='bicubic', align_corners=False)
            result = Image.fromarray(output.squeeze(0).permute(1,2,0).clamp(0,255).byte().cpu().numpy())
        else:
            result = result.resize((result.width*2, result.height*2), Image.Resampling.LANCZOS)
    return result


def split_page(image, options):
    if options.get('split') and image.width > image.height * 1.25:
        center = image.width // 2
        parts = [image.crop((0,0,center,image.height)), image.crop((center,0,image.width,image.height))]
        return parts[::-1] if options.get('rtl') else parts
    return [image]


def export_archive(pages, metadata, settings, excluded):
    buffer = io.BytesIO()
    mapping = {}
    with zipfile.ZipFile(buffer, 'w', zipfile.ZIP_DEFLATED) as archive:
        counter = 0
        for index, (_, image) in enumerate(pages):
            if index in excluded:
                continue
            mapping[index] = []
            for part in split_page(restore(image, settings[index]), settings[index]):
                mapping[index].append(counter)
                output = io.BytesIO()
                part.save(output, format='PNG')
                archive.writestr(f'{counter+1:04d}.png', output.getvalue())
                counter += 1
        if not counter:
            raise ValueError('Selecteer minimaal één pagina.')
        if metadata:
            if metadata:
                if b'<!DOCTYPE' in metadata.upper() or b'<!ENTITY' in metadata.upper():
                    raise ValueError('Onveilige XML in ComicInfo.xml')
                root = ET.fromstring(metadata)
                count = root.find('PageCount')
                if count is not None:
                    count.text = str(counter)
                page_nodes = root.find('Pages')
                if page_nodes is not None:
                    for node in list(page_nodes):
                        page_nodes.remove(node)
                        for new_index in mapping.get(int(node.get('Image', '-1')), []):
                            copy = ET.fromstring(ET.tostring(node))
                            copy.set('Image', str(new_index))
                            for attr in ('ImageSize', 'ImageWidth', 'ImageHeight', 'DoublePage'):
                                copy.attrib.pop(attr, None)
                            page_nodes.append(copy)
                metadata = ET.tostring(root, encoding='utf-8', xml_declaration=True)
            archive.writestr('ComicInfo.xml', metadata)
    return buffer.getvalue()
