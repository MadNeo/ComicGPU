'use strict';
const $ = id => document.getElementById(id);
const token = new URLSearchParams(location.search).get('token') || sessionStorage.getItem('comicgpu-token') || '';
sessionStorage.setItem('comicgpu-token', token);
history.replaceState(null, '', location.pathname);
const books = [];
let currentBook = 0;
let currentPage = 0;
let previewUrl;
let gpuAvailable = false;
let busy = false;
const downloads = [];
const definitions = [
  ['deskew', 'Automatisch rechtzetten', 'checkbox'],
  ['crop', 'Scanranden bijsnijden', 'checkbox'],
  ['grayscale', 'Zwart-wit', 'checkbox'],
  ['paper', 'Papierkleur opfrissen', 'range'],
  ['shadow', 'Rugschaduw verminderen', 'range'],
  ['bleed', 'Lichte doordruk verminderen', 'range'],
  ['moire', 'Moiré / raster verzachten', 'range'],
  ['upscale', '2× opschalen', 'checkbox'],
  ['gpu', 'GPU gebruiken bij opschalen', 'checkbox'],
  ['split', 'Brede pagina splitsen', 'checkbox'],
  ['rtl', 'Rechterhelft eerst (manga)', 'checkbox']
];
const emptyOptions = () => Object.fromEntries(definitions.map(([key, , type]) => [key, type === 'range' ? 0 : false]));
const selected = () => books[currentBook];
function status(message) { $('status').textContent = message; }
async function api(path, options = {}) {
  const response = await fetch('/api/' + path, {
    ...options,
    headers: { 'x-comicgpu-token': token, ...options.headers }
  });
  if (!response.ok) throw new Error(await response.text());
  return response;
}
const json = data => ({ method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(data) });
async function run(task) {
  if (busy) return;
  busy = true;
  const controls = [...document.querySelectorAll('button,input,select')];
  controls.forEach(control => control.disabled = true);
  try { await task(); } catch (error) { status(error.message); }
  finally {
    busy = false;
    controls.forEach(control => control.disabled = false);
    const gpu = $('option-gpu');
    if (gpu) gpu.disabled = !gpuAvailable;
  }
}
function clearDownloads() {
  downloads.forEach(url => URL.revokeObjectURL(url));
  downloads.length = 0;
  $('downloads').replaceChildren();
}
function changed() {
  selected().options[currentPage].approved = false;
  $('after').removeAttribute('src');
  clearDownloads();
  review();
  status('Instellingen gewijzigd. Werk de voorvertoning bij voordat je de pagina beoordeelt.');
}
function review() {
  const book = selected();
  $('review').replaceChildren();
  book.pages.forEach((page, index) => {
    const row = document.createElement('div');
    row.className = 'review-row' + (index === currentPage ? ' active' : '');
    row.textContent = `${index + 1} · ${page.name} · ${page.width}×${page.height} · ${page.review.join(', ') || 'Geen controlesignalen'}${book.options[index].approved ? ' · Beoordeeld' : ''}${book.options[index].excluded ? ' · Uitgesloten' : ''}`;
    row.tabIndex = 0;
    const choose = () => { if (busy) return; currentPage = index; showPage(); };
    row.onclick = choose;
    row.onkeydown = event => { if (event.key === 'Enter') choose(); };
    $('review').append(row);
  });
}
function showPage() {
  const book = selected();
  if (!book) { $('editor').hidden = true; return; }
  $('editor').hidden = false;
  $('pages').replaceChildren(...book.pages.map((page, index) => new Option(`${index + 1} · ${page.name}`, index)));
  $('pages').value = currentPage;
  const options = book.options[currentPage];
  $('excluded').checked = !!options.excluded;
  $('controls').replaceChildren();
  for (const [key, text, type] of definitions) {
    const label = document.createElement('label');
    const input = document.createElement('input');
    input.type = type;
    input.id = 'option-' + key;
    const caption = document.createElement('span');
    caption.textContent = text + (type === 'range' ? ` · ${Math.round(options[key] * 100)}%` : '');
    if (type === 'range') { input.min = 0; input.max = 1; input.step = .05; input.value = options[key]; }
    else { input.checked = options[key]; }
    if (key === 'gpu') input.disabled = !gpuAvailable;
    input.onchange = () => {
      options[key] = type === 'range' ? Number(input.value) : input.checked;
      caption.textContent = text + (type === 'range' ? ` · ${Math.round(options[key] * 100)}%` : '');
      changed();
    };
    label.append(input, caption);
    $('controls').append(label);
  }
  $('before').src = `/api/books/${book.id}/pages/${currentPage}?token=${encodeURIComponent(token)}`;
  $('after').removeAttribute('src');
  if (previewUrl) URL.revokeObjectURL(previewUrl);
  review();
  $('splitnote').textContent = '';
  status(book.warnings.length ? 'Archiefwaarschuwingen\n' + book.warnings.join('\n') : 'Kies je instellingen en werk de voorvertoning bij.');
}
function refreshBooks() {
  $('books').replaceChildren(...books.map((book, index) => new Option(book.name, index)));
  $('books').value = currentBook;
  showPage();
}
function addFiles(input) { return run(async () => {
  clearDownloads();
  const failures = [];
  for (const file of [...input.files].filter(file => /\.(cbz|cbr|zip)$/i.test(file.name))) {
    status(`Archief controleren · ${file.name}`);
    const form = new FormData(); form.append('file', file);
    try {
      const book = await (await api('books', { method: 'POST', body: form })).json();
      book.name = file.name;
      book.options = book.pages.map(emptyOptions);
      books.push(book);
    } catch (error) { failures.push(`${file.name}: ${error.message}`); }
  }
  input.value = ''; 
  currentBook = Math.max(0, books.length - 1); currentPage = 0;
  refreshBooks();
  if (failures.length) status(failures.join('\n'));
}); }
$('upload').onchange = () => addFiles($('upload'));
$('folder').onchange = () => addFiles($('folder'));
$('books').onchange = () => { currentBook = Number($('books').value); currentPage = 0; showPage(); };
$('pages').onchange = () => { currentPage = Number($('pages').value); showPage(); };
$('remove').onclick = () => run(async () => {
  if (!selected()) return;
  await api(`books/${selected().id}`, { method: 'DELETE' });
  books.splice(currentBook, 1); currentBook = 0; currentPage = 0;
  clearDownloads(); refreshBooks();
});
$('apply').onclick = () => {
  if (!selected()) return;
  const profile = $('profile').value;
  selected().options = selected().pages.map(() => ({ ...emptyOptions(), deskew: profile !== 'digital', crop: profile !== 'digital', paper: profile === 'scan' ? .35 : profile === 'manga' ? .25 : 0, grayscale: profile === 'manga' }));
  clearDownloads(); showPage();
};
$('excluded').onchange = () => { selected().options[currentPage].excluded = $('excluded').checked; changed(); };
$('preview').onclick = () => run(async () => {
  const book = selected();
  const page = currentPage;
  status('Voorvertoning berekenen…');
  const response = await api(`books/${book.id}/pages/${page}`, json(book.options[page]));
  const blob = await response.blob();
  if (previewUrl) URL.revokeObjectURL(previewUrl);
  previewUrl = URL.createObjectURL(blob);
  $('after').src = previewUrl;
  const split = book.options[page].split && book.pages[page].width > book.pages[page].height * 1.25;
  $('splitnote').textContent = split ? `Export splitst deze pagina in het midden. ${book.options[page].rtl ? 'Rechterhelft' : 'Linkerhelft'} wordt eerst opgeslagen.` : '';
  status('Voorvertoning gereed. Controleer tekst en tekenwerk.');
});
$('approve').onclick = () => {
  if (!$('after').hasAttribute('src')) { status('Werk eerst de voorvertoning bij.'); return; }
  selected().options[currentPage].approved = true; review(); status('Pagina beoordeeld.');
};
$('actual').onchange = () => document.querySelector('.comparison').classList.toggle('actual', $('actual').checked);
const viewports = [...document.querySelectorAll('.viewport')];
let syncing = false;
viewports.forEach((view, index) => view.addEventListener('scroll', () => {
  if (syncing) return;
  syncing = true;
  const other = viewports[1 - index];
  other.scrollLeft = view.scrollLeft / Math.max(1, view.scrollWidth - view.clientWidth) * (other.scrollWidth - other.clientWidth);
  other.scrollTop = view.scrollTop / Math.max(1, view.scrollHeight - view.clientHeight) * (other.scrollHeight - other.clientHeight);
  requestAnimationFrame(() => syncing = false);
}));
$('export').onclick = () => run(async () => {
  clearDownloads();
  if (!books.length) return;
  for (const [index, book] of books.entries()) {
    status(`Verwerken ${index + 1}/${books.length} · ${book.name}`);
    const response = await api(`books/${book.id}/export`, json({ options: book.options, workers: Number($('workers').value), allow_unreviewed: $('allow').checked }));
    const url = URL.createObjectURL(await response.blob());
    downloads.push(url);
    const link = document.createElement('a');
    link.href = url; link.download = book.name.replace(/\.(cbz|zip|cbr)$/i, '') + '_verbeterd.cbz';
    link.textContent = 'Download ' + link.download;
    $('downloads').append(link);
  }
  status('Batch voltooid. Download je verbeterde comics links.');
});
$('theme').onclick = () => {
  document.body.classList.toggle('light');
  localStorage.setItem('comicgpu-light', document.body.classList.contains('light'));
};
document.body.classList.toggle('light', localStorage.getItem('comicgpu-light') === 'true');
api('capabilities').then(response => response.json()).then(hardware => {
  gpuAvailable = !!hardware.gpu;
  $('hardware').textContent = `${hardware.cpus} CPU-threads · ${hardware.gpu || 'CPU-verwerking beschikbaar'}`;
  const input = $('option-gpu'); if (input) input.disabled = !gpuAvailable;
}).catch(error => status(error.message));
