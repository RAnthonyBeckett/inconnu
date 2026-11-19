#!/usr/bin/env python3
"""Extract the reviewed Phase 7C name, tier, and citation records."""
import argparse
from collections import Counter
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from extract_phase7a import key, load, slug, write
from extract_phase7b import LEVELS, ordered_columns, pdf_pages

PREFIX = 'phase7c-'
SOURCES = {
    'v20': 'V20 - Core Rulebook.pdf',
    'da20': 'Vampire_20th_Anniversary_Dark_Ages_(Final_Download).pdf',
    'lotn': 'WtWf-MET_LawsoftheNightRevised.pdf',
    'fera': 'W20 - Changing Breeds.pdf',
    'kuei': 'Kuei_Jin_-_Genre_-_2021.pdf',
    'fallen': 'Demon-Fallen Genre-2021.pdf',
    'hekau': 'Mummy-Magic-2024.pdf',
}
ADDITIONAL_SOURCES = {
    SOURCES['lotn']: ('f6eae9fda388c51e23bfe73338c867123655d5efa7957020857da7c370bac574', 273),
    SOURCES['kuei']: ('ad56626d8e3b50eab78f0826ca441c2bc0392a95a068aa62e34a980da44ada55', 79),
    SOURCES['fallen']: ('4c667c5d541248ef80b4af641e353be379e3d0a9469b6790ca1d6f6ef3836391', 237),
    SOURCES['hekau']: ('9b08e6e2feab8eef0c2c43cc1764c58ac0945cd81bbc31b33e21c0b952868680', 68),
}
TIER = ('Basic', 'Basic', 'Intermediate', 'Intermediate', 'Advanced')
CONVERTED = 'ConvertedFrom20thLevel'
NATIVE = 'NativeMetTier'

def checked(pages, rows):
    for family, name, rank, page, kind, target, price in rows:
        if key(name) not in key(pages[page]['text']):
            raise ValueError(f'Missing {name!r} on PDF leaf {page}')
        yield family, name, rank, page, kind, target, price

def vampire_rituals(pages, edition):
    start_ends = [(185, 192, 'Necromancy'), (238, 249, 'Thaumaturgy')] if edition == 'v20' else [
        (293, 296, 'Necromancy'), (304, 310, 'Thaumaturgy')]
    font = 'Delavan' if edition == 'v20' else 'DeRoosCaps'
    for start, end, family in start_ends:
        level = None
        for page in range(start, end + 1):
            for style, heading in ordered_columns(pages[page]):
                if font not in style:
                    continue
                match = re.match(r'level (one|two|three|four|five)\b', heading, re.I)
                if match:
                    level = LEVELS[match[1].title()]
                elif level and re.fullmatch(r"[A-Za-z][\w\s’'.,-]{2,69}", heading) and not re.search(
                    r'\brituals$|^other warding|^paths |^(?:obfuscate|vicissitude|body crafts|necromancy|thaumaturgy|gifts of the blood)$',
                    heading, re.I
                ):
                    name = re.sub(r"([’'])S\b", r"\1s", heading.title()) if edition == 'da20' else heading
                    yield family, name, level, page, 'ritual', 'RitualList', None

VAMPIRE_SERIES = {
    'v20': [
        ('The Cenotaph Path', 'path', [
            (174, 'A Touch of Death'), (174, 'Reveal the Catene'),
            (175, 'Tread Upon the Grave'), (175, 'Death Knell'), (176, 'Ephemeral Binding'),
        ]),
        ('The Path of Blood', 'path', [
            (221, 'A Taste for Blood'), (221, 'Blood Rage'), (221, 'Blood of Potency'),
            (222, 'Theft of Vitae'), (222, 'Cauldron of Blood'),
        ]),
        ('Temporis', 'discipline', [
            (475, 'Hourglass of the Mind'), (475, 'Recurring Contemplation'),
            (476, 'Leaden Moment'), (476, 'Patience of the Norns'), (477, "Clotho's Gift"),
        ]),
    ],
    'da20': [
        ('Daimonion', 'discipline', [
            (209, 'Sense the Sin'), (209, 'Fear of the Void'), (210, 'Tormented Essence'),
            (210, 'Psychomachia'), (210, 'Condemnation'),
        ]),
        ('Temporis', 'discipline', [
            (257, 'Hourglass of the Mind'), (258, 'Locked Contemplation'),
            (258, 'Leaden Movement'), (258, 'Patience of the Norns'), (259, "Clotho's Gift"),
        ]),
        ('The Path of the Sepulchre', 'path', [
            (288, 'Peering Across the Shroud'), (288, 'Summon Soul'), (288, 'Compel Soul'),
            (288, 'Haunting'), (289, 'Torment'),
        ]),
        ('The Path of Ash', 'path', [
            (289, 'Shroudsight'), (290, 'Tongue of the Dark Kingdoms'), (290, 'Touch the Nether'),
            (290, 'Ex Nihilo'), (290, 'Shroud Mastery'),
        ]),
    ],
}


def vampire_series(pages, edition):
    yield from checked(pages, (
        (family, name, rank, page, kind, None, None)
        for family, kind, powers in VAMPIRE_SERIES[edition]
        for rank, (page, name) in enumerate(powers, 1)
    ))


def native_vampire_rituals(pages):
    names = {
        (186, 1): ['Communicate with Kindred Sire', 'Defense of the Sacred Haven',
                   'Deflection of Wooden Doom', 'Devil’s Touch', 'The Open Passage'],
        (187, 1): ['Principal Focus of Vitae Infusion', 'Scent of the Lupine’s Passing',
                   'Wake with Evening’s Freshness', 'Ward versus Ghouls'],
        (187, 3): ['Bone of Lies', 'Incorporeal Passage'],
        (188, 3): ['Pavis of Foul Presence', 'Rutor’s Hands', 'Soul of the Homunculi'],
        (188, 5): ['Blood Contract', 'Nectar of the Bitter Rose'],
        (189, 5): ['Umbra Walk'],
    }
    yield from checked(pages, (
        ('Thaumaturgy', name, rank, page, 'ritual', 'RitualList', None)
        for (page, rank), entries in names.items() for name in entries
    ))

def vampire(pages, edition):
    if edition == 'lotn':
        yield from native_vampire_rituals(pages)
        return
    yield from vampire_rituals(pages, edition)
    yield from vampire_series(pages, edition)

def gurahl(pages):
    found = set()
    for page in range(115, 121):
        for _, line in ordered_columns(pages[page]):
            match = re.match(r'^\W\s+(.+?)\s*\(Level (One|Two|Three|Four|Five)\)', line)
            if match and (match[1], match[2]) not in found:
                found.add((match[1], match[2]))
                yield 'Gurahl', match[1], LEVELS[match[2]], page, 'gift', 'GiftList', None
    for name, rank, page in [
        ('Rite of Rending the Gauntlet', 1, 120),
        ('Rite of Replenishment', 2, 120),
        ('Rite of the River-Portent', 2, 120),
        ('Rite of True Mating', 2, 120),
        ('Rite of Fighting the Death Bear', 5, 121),
    ]:
        yield from checked(pages, [('Gurahl', name, rank, page, 'rite', 'RiteList', None)])

def shintai(pages):
    families = {'Smoke Shintai': (30, 31), 'Storm Shintai': (31, 32)}
    for family, (start, end) in families.items():
        active = False
        entries = []
        for page in range(start, end + 1):
            for font, line in ordered_columns(pages[page]):
                if 'Centaur' in font and line.startswith(family):
                    active = True
                elif active and 'Centaur' in font and not line.startswith(family):
                    active = False
                if active and (m := re.fullmatch(r'(Basic|Intermediate|Advanced): (.+)', line)):
                    entries.append((m[1], m[2], page))
        if tuple(tier for tier, _, _ in entries) != TIER:
            raise ValueError(f'Unexpected tier sequence for {family}')
        for rank, (_, name, page) in enumerate(entries, 1):
            yield family, name, rank, page, 'discipline', 'DisciplineList', None
    tiers = {'Basic': 1, 'Intermediate': 3, 'Advanced': 5, 'Expert': None}
    rank = None
    for page in range(63, 76):
        for line in pages[page]['text'].splitlines():
            line = line.strip()
            if line in tiers:
                rank = tiers[line]
            match = re.fullmatch(r"([A-Za-z][\w’' ,.-]{3,86}?)\s+\(.+?\bp\.?\s*\d+\)", line, re.I)
            if rank and match:
                yield 'Kuei-Jin Rites', match[1], rank, page, 'ritual', 'RitualList', None

def fallen(pages):
    lore = {
        'Fundament': [('Manipulate Gravity',48),('Manipulate Adhesion',49),('Manipulate Inertia',49),
                      ('Manipulate Acceleration',49),('Manipulate Cohesion',50)],
        'Humanity': [('Translate',51),('Insinuate',51)],
        'Celestials': [('Lamp of Faith',53),('Send Vision',53),('Pillar of Faith',54),('Fire of Heaven',54)],
    }
    lore_rows = [('Lore of '+family, name, rank, page, 'lore', 'LoreList', None)
                 for family, entries in lore.items() for rank,(name,page) in enumerate(entries,1)]
    yield from checked(pages, lore_rows)
    for page in range(148, 181):
        title = None
        primary = None
        for font, line in ordered_columns(pages[page]):
            if 'CambriaMath' in font:
                title, primary = line, None
            if title and (match := re.fullmatch(r'Primary Lore: (.+?)\s+([•●]{1,5})', line)):
                primary = (match[1], len(match[2]))
            if primary and title not in {'Devils', 'Scourges', 'Malefactors', 'Fiends', 'Defilers', 'Devourers', 'Slayers'} and (match := re.fullmatch(r'Base Cost: (\d+)\s*xp', line, re.I)):
                yield primary[0], title, primary[1], page, 'ritual', 'RitualList', int(match[1])
                title, primary = None, None

def mummy(pages):
    paths = [('Alchemy',10,20),('Amulets',21,28),('Celestial',29,37),
             ('Effigy',38,45),('Necromancy',46,56),('Nomenclature',57,68)]
    for family, start, end in paths:
        if key(family) not in key(pages[start]['text']):
            raise ValueError(f'Missing Hekau path heading {family}')
        # The packet lists tier prices for the paths themselves and enumerates their spells.
        for rank in range(1, 6):
            yield family, f'{family} {rank}', rank, start, 'hekau', 'HekauList', None
        rank = None
        for page in range(start, end + 1):
            for line in pages[page]['text'].splitlines():
                line = line.strip()
                if m := re.match(r'^\s*-(Basic|Intermediate|Advanced)-', line):
                    rank = {'Basic':1,'Intermediate':3,'Advanced':5}[m[1]]
                if rank and (m := re.search(r'^(?:-\w+-\s*)?([A-Z][\w\s’,\'\-]{2,78}):\s*Type:\s*(Spell|Ritual)(?=[,\s]|Difficulty)', line)):
                    yield family, m[1].strip(), rank, page, m[2].lower(), 'SpellList', None

def extract(args):
    published=load(args.catalogues/'published_powers.json')
    met=load(args.catalogues/'met_powers.json')
    published['entries']=[r for r in published['entries'] if not r['book_id'].startswith(PREFIX)]
    published['books']=[r for r in published['books'] if not r['id'].startswith(PREFIX)]
    met['entries']=[r for r in met['entries'] if not r['stable_id'].startswith(PREFIX)]
    menu_keys={key(n.get('name','')) for n in ET.parse(args.menu).getroot().iter()}
    stats=Counter()
    identities=set()
    configs=[
        ('v20','Vampire','V20','Vampire: The Masquerade 20th Anniversary Edition',8,CONVERTED,vampire),
        ('da20','Vampire','DA20','Vampire: The Dark Ages 20th Anniversary Edition',1,CONVERTED,vampire),
        ('lotn','Vampire','MET_Laws_of_the_Night_Revised','Laws of the Night Revised',1,NATIVE,vampire),
        ('fera','Fera','W20_Changing_Breeds','W20 Changing Breeds',1,CONVERTED,gurahl),
        ('kuei','KueiJin','OWbN_Kuei_Jin_2021','OWbN Kuei-Jin Genre',3,NATIVE,shintai),
        ('fallen','Demon','OWbN_Fallen_2021','OWbN Fallen Genre',3,NATIVE,fallen),
        ('hekau','Mummy','OWbN_Mummy_Magic_2024','OWbN Mummy Magic',None,NATIVE,mummy),
    ]
    for tag,scope,edition,title,offset,mapping,extractor in configs:
        digest,pages=pdf_pages(args,tag,SOURCES,ADDITIONAL_SOURCES)
        book_id=PREFIX+tag
        authority='BindingPacket' if edition.startswith('OWbN') else 'NonBindingOrBook'
        published['books'].append(dict(id=book_id,title=title,edition=edition,template_scope=scope,
                                       pdf_sha256=digest,coverage='partial',notes=[]))
        rows=extractor(pages,tag) if scope=='Vampire' else extractor(pages)
        for family,name,rank,page,kind,target,price in rows:
            if not re.search(r'[A-Za-z]',name) or len(name)>120 or any(ord(c)<32 for c in name):
                raise ValueError(f'{book_id}: invalid name {name!r}')
            stable_id=f'{book_id}.{slug(family)}.{slug(name)}.{rank}'
            if stable_id in identities:
                continue
            identities.add(stable_id)
            printed=page-offset if offset is not None else None
            path = family if kind == 'path' else None
            discipline = family
            if path:
                discipline = 'Thaumaturgy' if path == 'The Path of Blood' else 'Necromancy'
            published['entries'].append(dict(id=stable_id,book_id=book_id,name=name,discipline=discipline,
                path=path,kind=kind,level=rank,printed_page=printed,pdf_page=page,
                packet_authority=authority,evidence='page_text',tier_mapping_basis=mapping))
            stats[scope+'.published']+=1
            stats['legacy_menu_matches']+=key(name) in menu_keys
            if target:
                row=dict(stable_id=stable_id,template_scope=scope,target_list=target,family=family,branch=None,
                    name=name,tier=TIER[rank-1],purchase_family='Other',profile='ConfiguredCreature',
                    printed_page=printed or 0,pdf_page_1based=page,original_pdf_sha256=digest,
                    prerequisite_note='',source_reviewed=True,packet_authority=authority,
                    tier_mapping_basis=mapping,source_level=rank,published_source_id=stable_id)
                if price is not None:
                    row['fixed_xp_cost']=price
                met['entries'].append(row)
                stats[scope+'.met']+=1
    if args.write:
        write(args.catalogues/'published_powers.json',published)
        write(args.catalogues/'met_powers.json',met)
    print(dict(sorted(stats.items())))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ('cache','menu','locators','inventory','catalogues'):
        parser.add_argument(f'--{name}',required=True,type=Path)
    parser.add_argument('--pdf-root', type=Path)
    parser.add_argument('--write',action='store_true')
    extract(parser.parse_args())
