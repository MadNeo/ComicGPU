"""ComicGPU web interface. Start with streamlit run app.py."""
import hashlib
import io
import zipfile

import streamlit as st
from engine import PRESETS, read_archive, inspect_pages, restore, export_archive, split_page

st.set_page_config(page_title='ComicGPU', page_icon='📚', layout='wide')
st.title('ComicGPU')
st.caption('Herstel scans, controleer pagina’s en maak een verbeterde CBZ.')
uploads = st.file_uploader('Comics toevoegen', type=['cbz', 'zip', 'cbr'], accept_multiple_files=True)
if not uploads:
    st.info('Voeg een of meerdere comics toe. Kies daarna een profiel en beoordeel de voorvertoning.')
    st.stop()
identity = tuple((u.name, hashlib.sha256(u.getvalue()).hexdigest()) for u in uploads)
if st.session_state.get('identity') != identity:
    st.session_state.clear()
    st.session_state.identity = identity
    st.session_state.books = []
    for upload in uploads:
        try:
            pages, metadata, warnings = read_archive(upload.getvalue(), upload.name)
            st.session_state.books.append(dict(name=upload.name, pages=pages, metadata=metadata,
                warnings=warnings, report=inspect_pages(pages), settings=[{} for _ in pages], excluded=set(), approved=set()))
        except Exception as error:
            st.error(f'{upload.name}: {error}')
books = st.session_state.books
if not books:
    st.stop()
book_id = st.selectbox('Comic', range(len(books)), format_func=lambda i: books[i]['name'])
book = books[book_id]
for warning in book['warnings']:
    st.warning(warning)
with st.sidebar:
    st.header('Verwerkingsprofiel')
    profile = st.selectbox('Profiel', list(PRESETS), format_func=lambda p: {'digital':'Digitale comic','scan':'Oude kleurenscan','manga':'Zwart-wit / manga'}[p])
    if st.button('Profiel toepassen op deze comic'):
        book['settings'] = [dict(PRESETS[profile]) for _ in book['pages']]
        book['approved'].clear()
        st.session_state.pop('output', None)
        st.rerun()
    st.caption('Profielen zijn een startpunt. Controleer kleine tekst, rasterpunten en donkere tekeningen.')
review = [dict(Pagina=i+1, Bestand=r['name'], Controle=', '.join(r['review']) or 'Geen automatische signalen',
               Beoordeeld=i in book['approved']) for i,r in enumerate(book['report'])]
with st.expander('Controlelijst en paginavolgorde', expanded=True):
    st.dataframe(review, use_container_width=True, hide_index=True)
page_id = st.selectbox('Pagina bewerken', range(len(book['pages'])), format_func=lambda i: f"{i+1} · {book['pages'][i][0]}")
options = book['settings'][page_id]
with st.form(f'options-{book_id}-{page_id}'):
    a,b,c = st.columns(3)
    edited = {}
    with a:
        edited['deskew'] = st.checkbox('Automatisch rechtzetten (max. 3°)', value=options.get('deskew',False))
        edited['crop'] = st.checkbox('Uniforme scanranden bijsnijden', value=options.get('crop',False))
        edited['grayscale'] = st.checkbox('Zwart-wit', value=options.get('grayscale',False))
    with b:
        for key,label in [('paper','Papierkleur opfrissen'),('shadow','Rugschaduw verminderen'),('bleed','Lichte doordruk verminderen'),('moire','Moiré / raster verzachten')]:
            edited[key] = st.slider(label, 0.0, 1.0, float(options.get(key,0)), .05)
    with c:
        edited['upscale'] = st.checkbox('2× opschalen', value=options.get('upscale',False))
        edited['gpu'] = st.checkbox('CUDA gebruiken bij opschalen', value=options.get('gpu',False))
        edited['split'] = st.checkbox('Brede pagina splitsen', value=options.get('split',False))
        edited['rtl'] = st.checkbox('Rechterhelft eerst (manga)', value=options.get('rtl',False))
        exclude = st.checkbox('Pagina uitsluiten van uitvoer', value=page_id in book['excluded'])
    if st.form_submit_button('Instellingen opslaan en voorvertoning bijwerken'):
        book['settings'][page_id] = edited
        book['approved'].discard(page_id)
        if exclude:
            book['excluded'].add(page_id)
        else:
            book['excluded'].discard(page_id)
        st.session_state.pop('output', None)
        st.rerun()
original = book['pages'][page_id][1]
try:
    processed = restore(original, options)
    mode = st.radio('Voorvertoning', ['Volledige pagina', '100% detail'], horizontal=True)
    before, after = original, processed
    if mode == '100% detail':
        x = st.slider('Horizontale positie', 0.0, 1.0, .5)
        y = st.slider('Verticale positie', 0.0, 1.0, .5)
        def detail(image):
            width, height = min(480,image.width), min(480,image.height)
            left, top = int(x*(image.width-width)), int(y*(image.height-height))
            return image.crop((left,top,left+width,top+height))
        before, after = detail(original), detail(processed)
    left,right = st.columns(2)
    left.image(before, caption=f'Origineel · {original.width} × {original.height}', width=min(480,before.width))
    right.image(after, caption=f'Bewerkt · {processed.width} × {processed.height}', width=min(480,after.width))
    parts = split_page(processed,options)
    if len(parts)>1:
        st.caption('Uitvoervolgorde van de gesplitste pagina')
        st.image(parts, width=240)
    if st.button('Pagina beoordeeld'):
        book['approved'].add(page_id)
        st.rerun()
except Exception as error:
    st.error(f'Voorvertoning mislukt: {error}')
st.divider()
pending = sum(bool(r['review']) and i not in b['approved'] and i not in b['excluded'] for b in books for i,r in enumerate(b['report']))
st.write(f'{pending} pagina’s met controlesignalen wachten op beoordeling.')
allow = st.checkbox('Ook pagina’s met openstaande controlesignalen exporteren', value=False)
if st.button('Alle comics verwerken', type='primary', disabled=bool(pending and not allow)):
    try:
        result = io.BytesIO()
        progress = st.progress(0)
        with zipfile.ZipFile(result,'w',zipfile.ZIP_STORED) as batch:
            for i,b in enumerate(books):
                data = export_archive(b['pages'],b['metadata'],b['settings'],b['excluded'])
                safe_name = b['name'].replace('\\','/').split('/')[-1].rsplit('.',1)[0]
                batch.writestr(f'{i+1:03d}_{safe_name}_verbeterd.cbz',data)
                progress.progress((i+1)/len(books))
        st.session_state.output = result.getvalue()
        st.success('Verwerking voltooid. Download hieronder de verbeterde comics.')
    except Exception as error:
        st.error(f'Export afgebroken: {error}')
if 'output' in st.session_state:
    st.download_button('Verbeterde comics downloaden',st.session_state.output,'ComicGPU_resultaten.zip','application/zip')
