#!/usr/bin/env python3
"""Rebuild Phase 7B catalogue additions from reviewed PDF layout caches."""
import argparse
from collections import Counter, defaultdict
import hashlib
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from extract_phase7a import cache_pdf, key, load, slug, write

PREFIX = 'phase7b-'
LEVELS = dict(zip(('One', 'Two', 'Three', 'Four', 'Five', 'Six'), range(1, 7)))
GAROU_GROUPS = {
    'Homid', 'Metis', 'Lupus', 'Ragabash', 'Theurge', 'Philodox', 'Galliard', 'Ahroun',
    'Black Furies', 'Bone Gnawers', 'Children of Gaia', 'Fianna', 'Get of Fenris',
    'Glass Walkers', 'Red Talons', 'Shadow Lords', 'Silent Striders', 'Silver Fangs',
    'Stargazers', 'Uktena', 'Wendigo',
}
NATIVE_TIERS = ('Basic', 'Basic', 'Intermediate', 'Intermediate', 'Advanced')
SPHERES = {
    'Correspondence': (145, 146, 147, 147, 148),
    'Entropy': (149, 149, 150, 150, 151),
    'Forces': (151, 152, 152, 153, 154),
    'Life': (154, 155, 155, 156, 157),
    'Matter': (157, 158, 158, 159, 159),
    'Mind': (160, 161, 161, 162, 162),
    'Prime': (163, 164, 164, 165, 166),
    'Spirit': (167, 167, 168, 168, 169),
    'Time': (169, 170, 170, 171, 171),
}
SOURCES = {
    'w20': 'W20 - Core Rulebook.pdf',
    'ascension': 'Laws of Ascension (5022).pdf',
    'hunt': 'Laws of the Hunt (Revised Edition) (5032).pdf',
    'imbued': 'Hunter-Imbued-2021_0.pdf',
}
ADDITIONAL_SOURCES = {
    SOURCES['imbued']: ('b7e12baa65ea1ee04cb3f020fd77e714e6c633cbc1c4cfdbec19a497183f6a83', 41),
}
RISEN_DISCIPLINES = {'Celerity', 'Fortitude', 'Obfuscate', 'Potence'}

def pdf_pages(args, tag, sources=SOURCES, additional=ADDITIONAL_SOURCES):
    name = sources[tag]
    records = load(args.locators) + load(args.inventory)['reviewed_packet_evidence']
    identities = {(row['original_pdf_sha256'], row['pages']) for row in records
                  if row['file_name'] == name}
    if name in additional:
        identities.add(additional[name])
    cache_path = args.cache / f'{tag}.json'
    if args.pdf_root and not cache_path.exists():
        for path in sorted(args.pdf_root.rglob(name)):
            with path.open('rb') as stream:
                digest = hashlib.file_digest(stream, 'sha256').hexdigest()
            counts = {count for known, count in identities if known == digest}
            if len(counts) == 1:
                args.cache.mkdir(parents=True, exist_ok=True)
                write(cache_path, cache_pdf(path, tag, dict(original_pdf_sha256=digest, pages=counts.pop())))
                break
    if not cache_path.is_file():
        raise ValueError(f'{tag}: supply the layout cache or a PDF matching the reviewed hash')
    cached = load(cache_path)
    if cached['file'] != name or (cached['sha'], cached['pages']) not in identities:
        raise ValueError(f'{tag}: PDF cache does not match the reviewed identity')
    pages = {page['n']: page for page in cached['data']}
    return cached['sha'], pages

def ordered_columns(page):
    rows = defaultdict(list)
    for x, y, font, _, text in page['chunks']:
        if text.strip() and 10 < y < 815:
            rows[x >= 296, -round(y, 1)].append((x, font, text.splitlines()[0]))
    for _, fragments in sorted(rows.items()):
        fragments.sort(key=lambda fragment: fragment[0])
        yield fragments[0][1], ''.join(fragment[2] for fragment in fragments).strip()

def werewolf(pages):
    family = None
    for n in range(154, 204):
        for font, line in ordered_columns(pages[n]):
            heading = line.removesuffix(' Gifts')
            if 'Balthazar' in font and heading in GAROU_GROUPS:
                family = heading
            match = re.match(r'^•\s+(.+?)\s+\(Level (One|Two|Three|Four|Five|Six)\)', line)
            if match and family:
                yield family, match[1], LEVELS[match[2]], n, 'gift', None
    family = 'Rites of Accord'
    pending = None
    for n in range(205, 223):
        for font, line in ordered_columns(pages[n]):
            if 'Balthazar' in font:
                if line.startswith('Rites of ') or line.endswith(' Rites'):
                    family, pending = line, None
                elif line not in {'Learning a Rite', 'Auspice Roles'}:
                    pending = line
            match = re.fullmatch(r'Level (\w+)', line)
            if pending and match and match[1] in LEVELS:
                yield family, pending, LEVELS[match[1]], n, 'rite', None
                pending = None
    for n in (385, 386):
        for _, line in ordered_columns(pages[n]):
            match = re.match(r'^•\s+(.+?)\s+\(Level (One|Two|Three|Four|Five)\)', line)
            if match:
                yield 'Kinfolk', match[1], LEVELS[match[2]], n, 'gift', None


def mage_spheres(pages):
    for family, anchors in SPHERES.items():
        for level, (rank, n) in enumerate(zip(('Apprentice', 'Initiate', 'Disciple', 'Adept', 'Master'), anchors), 1):
            if not pages[n]['text'].strip():
                raise ValueError(f'Missing reviewed {family} {rank} leaf {n}')
            yield family, f'{family} — {rank}', level, n, 'sphere', None, None

def exact_rows(pages, entries):
    for family, name, level, pdf, kind, fixed in entries:
        if key(name) not in key(pages[pdf]['text']):
            raise ValueError(f'Check the {kind} heading {name!r} on PDF leaf {pdf}')
        yield family, name, level, pdf, kind, None, fixed

MORTAL_NUMINA = (
    ('Animal Communication', 'Communication', 1, 164, 'numina', None),
    ('Animal Communication', 'Mass Communication', 3, 164, 'numina', None),
    ('Animal Communication', 'Domination', 5, 164, 'numina', None),
    ('Astral Projection', 'Seeker', 1, 166, 'numina', None),
    ('Astral Projection', 'Journeyman', 3, 166, 'numina', None),
    ('Astral Projection', 'Wayfarer', 5, 167, 'numina', None),
    ('Biocontrol', 'Self Control', 1, 167, 'numina', None),
    ('Biocontrol', 'Hysterical Strength', 3, 167, 'numina', None),
    ('Biocontrol', 'Mind Over Matter', 5, 168, 'numina', None),
    ('Channeling', 'Channeler', 1, 169, 'numina', None),
    ('Channeling', 'Medium', 3, 169, 'numina', None),
    ('Channeling', 'Necromancer', 5, 170, 'numina', None),
    ('Clairvoyance', 'Secret Glimpses', 1, 170, 'numina', None),
    ('Clairvoyance', 'Sense of the Beyond', 3, 171, 'numina', None),
    ('Clairvoyance', 'Cosmic Awareness', 5, 171, 'numina', None),
    ('Cyberkinesis', 'Switch', 1, 172, 'numina', None),
    ('Cyberkinesis', 'Scramble', 3, 173, 'numina', None),
    ('Cyberkinesis', 'Run Hot', 5, 173, 'numina', None),
    ('Psychic Healing', 'Psychic Surgery', 5, 180, 'numina', None),
    ('Psychic Hypnosis', 'Trance State', 1, 180, 'numina', None),
    ('Psychic Hypnosis', 'Delayed Action', 3, 181, 'numina', None),
    ('Psychic Hypnosis', 'Manchurian Candidate', 5, 181, 'numina', None),
    ('Psychic Invisibility', 'Hide', 1, 182, 'numina', None),
    ('Psychic Invisibility', 'Invisibility', 3, 182, 'numina', None),
    ('Psychometry', 'Emotional Connection', 1, 186, 'numina', None),
    ('Psychometry', 'Time and Place', 3, 187, 'numina', None),
    ('Psychometry', 'Touch the Soul', 5, 187, 'numina', None),
)

MORTAL_RITUALS = (
    ('Alchemy', 'Ointment of Maedb', 2, 203, 'ritual', None),
    ('Alchemy', 'Powdered Truth', 3, 203, 'ritual', None),
    ('Alchemy', 'Tempering Balm', 5, 203, 'ritual', None),
    ('Binding', 'Pressing the Blood Servant', 4, 219, 'ritual', None),
)

IMBUED_EDGES = (
    ('Divine Edges', 'Cremate', 5, 31, 'edge', None),
    ('Divine Edges', 'Denounce', 5, 32, 'edge', None),
    ('Divine Edges', 'Infuse', 5, 32, 'edge', None),
    ('Corrupt Edges', 'Channel', 5, 36, 'edge', None),
    ('Corrupt Edges', 'Enthrall', 5, 37, 'edge', None),
    ('Corrupt Edges', 'Transport', 5, 37, 'edge', None),
)

ROTES = (
    ('Spirit', 'Animate the Dead', 2, 171, 'rote', 2),
    ('Prime', 'Block Magical Energy', 2, 172, 'rote', 2),
    ('Matter and Prime', 'Create Normal Item', 2, 172, 'rote', 4),
    ('Entropy', 'Decrepify', 4, 172, 'rote', 4),
    ('Forces', 'Deflect Bullets', 2, 172, 'rote', 2),
    ('Forces and Prime', 'Enhance Weapon', 3, 172, 'rote', 5),
)

def extract(args):
    met = load(args.catalogues / 'met_powers.json')
    published = load(args.catalogues / 'published_powers.json')
    met['entries'] = [row for row in met['entries'] if not row['stable_id'].startswith(PREFIX)]
    published['entries'] = [row for row in published['entries'] if not row['book_id'].startswith(PREFIX)]
    published['books'] = [row for row in published['books'] if not row['id'].startswith(PREFIX)]
    menu_names = {key(node.get('name', '')) for node in ET.parse(args.menu).getroot().iter()}
    report = Counter()
    ids = set()

    def book(tag, title, edition, scope, offset, rows, mapping, authority='NonBindingOrBook'):
        digest, _ = pdf_pages(args, tag)
        book_id = PREFIX + tag
        published['books'].append(dict(id=book_id, title=title, edition=edition, template_scope=scope,
                                       pdf_sha256=digest, coverage='partial', notes=[]))
        for family, name, level, pdf, kind, branch, *price in rows:
            ident = f'{book_id}.{slug(family)}.{slug(name)}.{level}'
            if ident in ids or len(name) > 120 or not re.search(r'[A-Za-z]', name) or any(ord(c) < 32 for c in name):
                raise ValueError(f'{book_id}: duplicate or invalid heading {name!r}')
            ids.add(ident)
            report[kind] += 1
            report['legacy_menu_matches'] += key(name) in menu_names
            row = dict(id=ident, book_id=book_id, name=name, discipline=family, path=branch,
                       kind=kind, level=level, printed_page=pdf-offset if offset is not None else None,
                       pdf_page=pdf, packet_authority=authority, evidence='page_text')
            if mapping and level <= 5:
                row['tier_mapping_basis'] = mapping
            published['entries'].append(row)
            target = {'gift':'GiftList', 'rite':'RiteList', 'sphere':'SphereList',
                      'rote':'RoteList', 'numina':'NuminaList', 'ritual':'RitualList'}.get(kind)
            if target and level <= 5 and family != 'Kinfolk':
                add_met(row, digest, scope, target, NATIVE_TIERS[level-1], next(iter(price), None))

    def add_met(row, digest, scope, target, tier, fixed=None):
        entry = dict(stable_id=PREFIX + row['id'] if not row['id'].startswith(PREFIX) else row['id'],
                     template_scope=scope, target_list=target, family=row['discipline'],
                     branch=row['path'], name=row['name'], tier=tier, purchase_family='Other',
                     profile='ConfiguredCreature', printed_page=row['printed_page'] or 0,
                     pdf_page_1based=row['pdf_page'], original_pdf_sha256=digest,
                     prerequisite_note='', source_reviewed=True, packet_authority=row['packet_authority'],
                     source_level=row['level'], published_source_id=row['id'])
        if basis := row.get('tier_mapping_basis'):
            entry['tier_mapping_basis'] = basis
        if fixed is not None:
            entry['fixed_xp_cost'] = fixed
        met['entries'].append(entry)

    wr20 = next(book for book in published['books'] if book['id'] == 'phase7a-wr20-wraith')
    for row in published['entries']:
        if row['book_id'] == wr20['id'] and row['kind'] == 'thorn':
            add_met(row, wr20['pdf_sha256'], 'Wraith', 'ThornList', 'Basic', row['level'])
            report['shadow_thorn_definition'] += 1
    native = [row for row in met['entries'] if row['profile'] == 'MetLawsOfTheNightRevised'
              and row['family'] in RISEN_DISCIPLINES]
    if Counter(row['family'] for row in native) != Counter(dict.fromkeys(RISEN_DISCIPLINES, 5)):
        raise ValueError('The reviewed native Risen Discipline list differs')
    for row in native:
        fields = ('branch', 'family', 'name', 'original_pdf_sha256', 'pdf_page_1based', 'printed_page', 'tier')
        met['entries'].append(dict(
            {field: row[field] for field in fields},
            stable_id=f"{PREFIX}risen.{row['stable_id']}", native_source_id=row['stable_id'],
            profile='ConfiguredCreature', purchase_family='Discipline', template_scope='Risen',
            target_list='DisciplineList', source_reviewed=True, packet_authority='BindingPacket',
            tier_mapping_basis='NativeMetTier', prerequisite_note='Use the Risen packet eligibility rules.'))
        report['risen_discipline_definition'] += 1

    _, w20 = pdf_pages(args, 'w20')
    book('w20', 'Werewolf: The Apocalypse 20th Anniversary Edition', 'W20', 'Werewolf',
         2, werewolf(w20), 'ConvertedFrom20thLevel')
    _, ascension = pdf_pages(args, 'ascension')
    book('ascension', 'Laws of Ascension', 'MET_Laws_of_Ascension', 'Mage',
         2, (*mage_spheres(ascension), *exact_rows(ascension, ROTES)), 'NativeMetTier')
    _, hunt = pdf_pages(args, 'hunt')
    book('hunt', 'Laws of the Hunt (Revised)', 'MET_Laws_of_the_Hunt_Revised', 'Mortal',
         2, exact_rows(hunt, (*MORTAL_NUMINA, *MORTAL_RITUALS)), 'NativeMetTier')
    _, imbued = pdf_pages(args, 'imbued')
    book('imbued', 'OWbN Hunter Imbued Packet', 'OWbN_Imbued_2021', 'Hunter',
         3, exact_rows(imbued, IMBUED_EDGES), None, 'BindingPacket')
    if args.write:
        write(args.catalogues / 'published_powers.json', published)
        write(args.catalogues / 'met_powers.json', met)
    print(dict(sorted(report.items())))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cache', 'menu', 'locators', 'inventory', 'catalogues'):
        parser.add_argument(f'--{name}', required=True, type=Path)
    parser.add_argument('--pdf-root', type=Path, help='Rebuild missing local caches from reviewed PDFs')
    parser.add_argument('--write', action='store_true')
    extract(parser.parse_args())
