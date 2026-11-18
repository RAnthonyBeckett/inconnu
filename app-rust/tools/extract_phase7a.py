#!/usr/bin/env python3
"""Build Phase 7A name/level/page catalogues from local PDF layout evidence.

The cache uses pypdf page text and visitor fragments (x, y, font, size, text).
Only verified headings, edition identifiers, hashes and page coordinates leave it.
"""
import argparse
import hashlib
from collections import Counter, defaultdict
import json
from pathlib import Path
import re
import unicodedata
import xml.etree.ElementTree as ET

STYGIAN = 'Argos Castigate Embody Fatalism Flux Inhabit Intimation Keening Lifeweb Mnemosynis Moliate Outrage Pandemonium Phantasm Puppetry Usury'.split()
DARK = 'Collogue Contaminate Corruption Larceny Maleficence'.split() + ['Shroud Rending', 'Tempestos', 'Tempest Weaving']
REGIONAL = ['The Way of the Scholar', 'The Way of the Artisan', 'The Way of the Farmer', 'The Way of the Merchant',
            'The Way of the Soul', 'Chains of the Emperor', 'Arcanoi of Svarga: Displace',
            'Arcanoi of the Bush of Ghosts: Behest']
C20 = '''
Autumn|19:Creeping Shadows|19:Autumn Eyes|19:The Poisoned Apple|19:The Withering|20:Shivers
Chicanery|20:Trick of the Light|20:Veiled Eyes|21:Dream Logic|21:Veiled Mind|22:Lost in the Mists
Chronos|22:Backward Glance|23:Effect and Cause|23:Set in Stone|24:Deja vu|24:Time Dilation
Contract|25:Done Deal|25:Liar’s Bell|25:Castigate|25:Casual Contract|25:Sanctified Words
Dragon’s Ire|27:Burning Thew|27:Confounding Coils|27:Dragonscales|27:Holly-Strike|27:Tripping the Ire
Legerdemain|27:Ensnare|27:Mooch|28:Effigy|28:Gimmix|28:Smoke and Mirrors
Metamorphosis|29:Sparrows and Nightingales|30:Worms and Giants|30:Thousandskins|31:Beastskin|32:Chimerical Exultation
Naming|32:Between the Lines|32:Nickname|33:Saining|33:Runic Imprint|33:Reweaving
Oneiromancy|35:Dream Walk|35:Dream Craft|35:Dream Portal|35:Manifest|36:Dream Puppet
Primal|36:Willow Whisper|37:Eldritch Prime|37:Oakenshield|37:Elder-Form|38:Dance of the Five Kings
Pyretics|38:Kindle|38:Illuminate|38:Purify|39:Engulf|39:Phoenix Song
Skycraft|39:Howling Gale|40:Electric Gremlins|40:Hurricane Speed|40:Storm Shroud|40:Lord of Levin
Soothsay|41:Omen|41:Seer’s Wisps|41:Tattletale|41:Augury|41:Fate Fire
Sovereign|42:Protocol|42:Grandeur|42:Guest List|43:Dictum|43:Geas
Spring|44:Awaken|44:Verdant Reclamation|44:Well of Life|45:Faerie Ring|45:Renewal
Summer|46:Flicker-flies|46:Enkindle|46:Aphrodesia|46:Vesta’s Blessing|47:The Beltane Blade
Wayfare|47:Hopscotch|47:Quicksilver|47:Portal Passage|48:Wind Runner|48:Flickerflash
Winter|48:Chill|48:Hardened Heart|49:Terror of the Long Night|49:Sculpt|49:Stasis
Actor|50:True Friend|50:Personal Contact|50:Familiar Face|50:Dire Enemy|50:Complete Stranger
Fae|51:Hearty Commoner|51:Lofty Noble|51:Manifold Chimera|51:Elusive Gallain|51:Dweomer of Glamour
Nature|51:Base Element|51:Raw Material|51:Verdant Forest|51:Feral Animal|51:Natural Phenomenon
Prop|51:Ornate Garb|51:Crafted Tool|51:Mechanical Device|51:Complex Machine|51:Arcane Artifact
Scene|52:Chamber|52:Cottage|52:Lonely Lane|52:Glen|52:Castle
Time|52:Three-Fold|52:Suspended Sands|52:Fateful Trigger|52:Bright Echoes|52:Glorious Renewal
'''
SOURCE_FILES = {
    'wr20': 'wraith-the-oblivion-20th-anniversary.pdf',
    'arcanoi': 'Wraith-Arcanoi-2016_0.pdf',
    'risen': 'Wraith-Risen-2019_0.pdf',
    'c20-packet': 'Changeling-Changeling 20th Anniversary Translations-2018_0.pdf',
}
RISEN_MET = '''
Fascinate|47:Distraction|48:Remembrance|48:Charge of Duty|48:Driving Urge|49:Target Lock
'''
RISEN = '''
Serendipity|11:Fortuitous Wandering|11:Flashback|11:The Face in the Crowd|11:Fortuitous Meeting|12:The Coincidental Gathering
'''

def key(value):
    folded = unicodedata.normalize('NFKD', value.casefold())
    return ''.join(ch for ch in folded if ch.isascii() and ch.isalnum())

def slug(value):
    folded = unicodedata.normalize('NFKD', value.casefold()).encode('ascii', 'ignore').decode()
    return re.sub(r'[^a-z0-9]+', '-', folded).strip('-')

def load(path):
    return json.loads(path.read_text(encoding='utf-8-sig'))

def write(path, value):
    path.write_text(json.dumps(value, indent=2, ensure_ascii=False) + '\n', encoding='utf-8', newline='\n')

def lines(page):
    groups = defaultdict(list)
    for x, y, font, _, text in page['chunks']:
        if text.strip():
            groups[round(y, 1), x < 296].append((x, font, text.splitlines()[0]))
    for (_, _), parts in groups.items():
        first = parts[0]
        yield first[1], ''.join(part[2] for part in parts).strip()

def manifest(text, pages, realm_names=()):
    for group in text.strip().splitlines():
        family, *entries = group.split('|')
        assert len(entries) == 5, family
        for level, entry in enumerate(entries, 1):
            n, name = entry.split(':', 1)
            n = int(n)
            if key(name) not in key(pages[n]['text']):
                raise ValueError(f'Missing verified heading: {n} {family}: {name}')
            yield family, name, level, n, 'realm' if family in realm_names else 'art', None

def wr20_arcanoi(pages, start, end, families):
    family = None
    headings = {key(name): name for name in families}
    for n in range(start, end + 1):
        for font, text in lines(pages[n]):
            if 'MatrixTall' not in font:
                continue
            if key(text) in headings:
                family = headings[key(text)]
            if family and (heading := re.match(r'^(•{1,5})\s*(.+)$', text)):
                level, name = heading.groups()
                if name == '•':
                    continue
                branch = re.search(r'\s+\((Common|Initiate)\)$', name)
                if branch:
                    name = name[:branch.start()]
                yield family, name, len(level), n, 'arcanos', branch[1] if branch else None

def thorns(pages):
    for n in range(296, 300):
        for _, _, font, _, text in pages[n]['chunks']:
            text = text.strip()
            match = re.fullmatch(r'(.+?): ([1-9])(?: to ([1-9]))? points?', text)
            if 'GoudyStd-Bold' in font and match:
                name, low, high = match.groups()
                for price in range(int(low), int(high or low) + 1):
                    yield 'Shadow Thorns', name, price, n, 'thorn', 'Freebie cost'

def met_arcanoi(pages):
    families = {key(name): name for name in STYGIAN}
    family = None
    tier = None
    index = 0
    for n, page in sorted(pages.items()):
        if n < 4:
            continue
        for x, _, font, _, text in page['chunks']:
            text = text.strip()
            header = re.fullmatch(r'(.+?)\s+(?:[1-9]|1[0-6])', text)
            if text == 'Contaminate' and x < 60 and 'Bold' in font:
                return
            if ('Bold' in font and header and key(header[1]) in families
                    and len(text) < 35):
                family = families[key(header[1])]
                tier = None
            match = re.fullmatch(r'(Basic|Intermediate|Advanced)\s*\(\s*(?:4|7|10)\s*xp(?: each)?\s*\)', text, re.I)
            if family and match:
                tier = match[1].capitalize()
                index = 0
                continue
            if re.match(r'^Innate Abilit', text):
                tier = None
            if tier and 'Bold' in font and x < 65 and re.fullmatch(r'[^:;.!?]{2,52}:', text):
                level = {'Basic':1, 'Intermediate':3, 'Advanced':5}[tier] + index
                if level <= {'Basic':2, 'Intermediate':4, 'Advanced':5}[tier]:
                    yield family, text[:-1].strip(), level, n, 'arcanos', None
                    index += 1


def dark_packet_arcanoi(pages):
    families = {key(name): name for name in DARK}
    families[key('Hive - Mind')] = 'Hive-Mind'
    family = None
    tier = None
    index = 0
    for n in range(33, 47):
        for x, _, font, _, text in pages[n]['chunks']:
            text = text.strip()
            if x < 60 and 'Bold' in font and key(text) in families and not text.endswith(':'):
                family = families[key(text)]
                tier = None
            header = re.fullmatch(r'(Basic|Intermediate|Advanced):\s*(?:4|7|10)\s*xp', text, re.I)
            if family and header:
                tier = header[1].capitalize()
                index = 0
                continue
            if tier and 'Bold' in font and x < 65 and re.fullmatch(r'[^:;.!?]{2,52}:', text):
                level = {'Basic':1, 'Intermediate':3, 'Advanced':5}[tier] + index
                if level <= {'Basic':2, 'Intermediate':4, 'Advanced':5}[tier]:
                    yield family, text[:-1].strip(), level, n, 'arcanos', None
                    index += 1


def cache_pdf(path, source_tag, evidence):
    from pypdf import PdfReader

    with path.open('rb') as source:
        digest = hashlib.file_digest(source, 'sha256').hexdigest()
    pdf = PdfReader(path, strict=False)
    if digest != evidence['original_pdf_sha256'] or len(pdf.pages) != evidence['pages']:
        raise ValueError(f'{path.name}: local PDF disagrees with Phase 1 evidence')
    pages = []
    for n, page in enumerate(pdf.pages, 1):
        if source_tag == 'wr20' and not (214 <= n <= 299 or 414 <= n <= 423
                                           or 461 <= n <= 463 or 522 <= n <= 532):
            continue
        chunks = []
        def visitor(text, _, matrix, font, size):
            if text.strip():
                chunks.append([round(matrix[4], 1), round(matrix[5], 1),
                               str((font or {}).get('/BaseFont', '')), round(size, 1), text])
        text = page.extract_text(visitor_text=visitor)
        pages.append(dict(n=n, text=text, chunks=chunks))
    return dict(sha=digest, pages=len(pdf.pages), file=path.name, data=pages)


def extract(args):
    locators = {row['file_name']: row for row in load(args.locators)}
    evidence = {row['file_name']: row for row in load(args.inventory)['reviewed_packet_evidence']}
    menu_names = {key(node.get('name', '')) for node in ET.parse(args.menu).getroot().iter() if node.get('name')}
    known = {**locators, **evidence}
    sources = {}
    for tag, name in SOURCE_FILES.items():
        cached = args.cache / f'{tag}.json'
        if args.pdf_root and not cached.exists():
            matches = list(args.pdf_root.rglob(name))
            verified = []
            for path in matches:
                with path.open('rb') as stream:
                    digest = hashlib.file_digest(stream, 'sha256').hexdigest()
                if digest == known[name]['original_pdf_sha256']:
                    verified.append(path)
            if not verified:
                raise ValueError(f'{name}: no PDF matches the Phase 1 hash')
            args.cache.mkdir(parents=True, exist_ok=True)
            write(cached, cache_pdf(sorted(verified)[0], tag, known[name]))
        source = load(cached)
        if source['file'] != name:
            raise ValueError(f'{tag}: cache identity differs from the expected title')
        recorded = known[source['file']]
        if (source['sha'], source['pages']) != (recorded['original_pdf_sha256'], recorded['pages']):
            raise ValueError(f'{tag}: cache disagrees with Phase 1 evidence')
        sources[tag] = source, {row['n']: row for row in source['data']}
    destination = args.catalogues
    published = load(destination / 'published_powers.json')
    met = load(destination / 'met_powers.json')
    prefix = 'phase7a-'
    published['books'] = [book for book in published['books'] if not book['id'].startswith(prefix)]
    published['entries'] = [row for row in published['entries'] if not row['book_id'].startswith(prefix)]
    met['entries'] = [row for row in met['entries'] if not row['stable_id'].startswith(prefix)]
    report = {}
    def add(tag, scope, edition, title, offset, rows, authority, mapping, met_enabled=True):
        source, _ = sources[tag]
        book = f'{prefix}{tag}-{scope.lower()}'
        published['books'].append(dict(id=book, title=title, edition=edition, pdf_sha256=source['sha'],
                                       template_scope=scope, coverage='partial', notes=[]))
        ids = set()
        for family, name, level, pdf, kind, path in rows:
            ident = f'{book}.{slug(family)}.{slug(name)}.{level}'
            if ident in ids or not re.search(r'[A-Za-z]', name) or any(ord(ch) < 32 for ch in name):
                raise ValueError(f'{book}: invalid or duplicate heading {name!r}')
            ids.add(ident)
            report.setdefault(book, Counter())[kind] += 1
            report[book]['legacy_menu_name_matches'] += key(name) in menu_names
            tier_basis = mapping if kind != 'thorn' else None
            printed = None if offset is None else pdf - offset
            row = dict(id=ident, book_id=book, name=name, discipline=family, path=path, kind=kind,
                       level=level, printed_page=printed, pdf_page=pdf, evidence='page_text',
                       packet_authority=authority)
            if tier_basis:
                row['tier_mapping_basis'] = tier_basis
            published['entries'].append(row)
            if met_enabled and kind != 'thorn':
                target = {'art':'ArtList', 'realm':'RealmList', 'arcanos':'ArcanoiList', 'discipline':'DisciplineList'}[kind]
                entry = dict(stable_id=ident, template_scope=scope, target_list=target, family=family,
                             branch=path, name=name, tier='Basic' if level <= 2 else 'Intermediate' if level <= 4 else 'Advanced',
                             purchase_family='Other' if kind != 'discipline' else 'Discipline',
                             profile='ConfiguredCreature', printed_page=printed or 0, pdf_page_1based=pdf,
                             original_pdf_sha256=source['sha'], prerequisite_note='', source_reviewed=True,
                             packet_authority=authority, tier_mapping_basis=tier_basis, source_level=level,
                             published_source_id=ident)
                if tag in ('risen', 'arcanoi'):
                    entry['fixed_xp_cost'] = [4,4,7,7,10][level-1]
                met['entries'].append(entry)
    wp = sources['wr20'][1]
    stygian = list(wr20_arcanoi(wp, 214, 285, STYGIAN))
    dark = list(wr20_arcanoi(wp, 414, 423, DARK))
    if Counter(row[0] for row in stygian) != Counter(dict.fromkeys(STYGIAN, 10)):
        raise ValueError('Stygian Arcanoi headings differ from the reviewed layout')
    if Counter(row[0] for row in dark) != Counter(dict.fromkeys(DARK, 5)):
        raise ValueError('Dark Arcanoi headings differ from the reviewed layout')
    regional = list(wr20_arcanoi(wp, 522, 532, REGIONAL))
    risen_wr20 = list(wr20_arcanoi(wp, 461, 463, ['Fascinate', 'Serendipity']))
    add('wr20', 'Wraith', 'Wr20', 'Wraith: The Oblivion 20th Anniversary Edition', 65,
        stygian + dark + regional + list(thorns(wp)), 'NonBindingOrBook', 'ConvertedFrom20thLevel')
    add('wr20', 'Risen', 'Wr20', 'Wraith: The Oblivion 20th Anniversary Edition', 65,
        risen_wr20, 'NonBindingOrBook', 'ConvertedFrom20thLevel')
    add('c20-packet','Changeling','OWbN_C20_2018','OWbN Changeling 20th Anniversary MET Translations', 2,
        manifest(C20, sources['c20-packet'][1], ('Actor','Fae','Nature','Prop','Scene','Time')), 'NonBindingOrBook','PacketSpecified')
    packet_arcanoi = list(met_arcanoi(sources['arcanoi'][1]))
    if Counter(row[0] for row in packet_arcanoi) != Counter(dict.fromkeys(STYGIAN, 5)):
        raise ValueError('MET Arcanoi packet headings differ from the reviewed layout')
    dark_packet = list(dark_packet_arcanoi(sources['arcanoi'][1]))
    if Counter(row[0] for row in dark_packet) != Counter(dict.fromkeys(['Hive-Mind', *DARK[1:]], 5)):
        raise ValueError('Dark MET Arcanoi packet headings differ from the reviewed layout')
    add('arcanoi', 'Wraith', 'OWbN_Arcanoi_2016', 'OWbN Arcanoi Packet', None,
        packet_arcanoi + dark_packet, 'NonBindingOrBook', 'NativeMetTier')
    add('arcanoi', 'Risen', 'OWbN_Arcanoi_2016', 'OWbN Arcanoi Packet', None,
        ((f,n,l,p,'arcanos',b) for f,n,l,p,_,b in manifest(RISEN_MET, sources['arcanoi'][1])),
        'NonBindingOrBook', 'NativeMetTier')
    add('risen','Risen','OWbN_Risen_2019','OWbN Risen Packet', None,
        ((f,n,l,p,'arcanos',b) for f,n,l,p,_,b in manifest(RISEN, sources['risen'][1])), 'BindingPacket','PacketSpecified')
    if args.write:
        write(destination / 'published_powers.json', published)
        write(destination / 'met_powers.json', met)
    print(json.dumps(report, indent=2, sort_keys=True))

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cache','menu','locators','inventory','catalogues'):
        parser.add_argument(f'--{name}', required=True, type=Path)
    parser.add_argument('--pdf-root', type=Path, help='Rebuild missing local caches with pypdf')
    parser.add_argument('--write', action='store_true')
    extract(parser.parse_args())

if __name__ == '__main__':
    main()
