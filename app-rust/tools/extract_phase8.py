"""Build the local Imbued baseline from the verified 2021 packet and Grapevine menu."""
import argparse
from pathlib import Path
import re
import xml.etree.ElementTree as ET

from extract_phase7b import IMBUED_EDGES, NATIVE_TIERS, exact_rows, pdf_pages
from extract_phase7a import key, load, slug, write

MENU_BOOK = 'phase8-imbued-menu'
PACKET_BOOK = 'phase7b-imbued'
EDITION = 'OWbN_Imbued_2021'
AUTHORITY = 'NonBindingOrBook'
BASIS = 'PacketSpecified'
RANKS = dict(zip(('touched', 'gifted', 'devoted', 'inspired', 'exalted'), range(1, 6)))
CONTEXT_LEAF = 9
RULES_NOTE = (
    'Laws of the Reckoning (#5037) page citations are pending. '
    'This local baseline uses the 2021 OWbN Imbued packet for context and the legacy Grapevine '
    'menu for Creed Edge names and ranks. Both lists use the adopted 3/6/9 XP schedule '
    '(+1 out of affiliation). Confirm local adoption, starting resources, and Edge eligibility '
    'with a Storyteller.'
)


def creed_edges(menu):
    root = ET.parse(menu).getroot()
    index = root.find("./menu[@name='Edges'][@category='12']")
    if index is None:
        raise ValueError('Missing Hunter Edges menu')
    for submenu in index.findall('submenu'):
        name = submenu.attrib['name']
        branch = root.find(f"./menu[@name='{name}'][@category='12']")
        if branch is None:
            raise ValueError(f'Missing Creed Edge branch {name}')
        ranks = set()
        for item in branch.findall('item'):
            rank = RANKS[item.attrib['note']]
            power = item.attrib['name']
            if not power.strip() or len(power) > 120 or not re.search(r'[A-Za-z]', power):
                raise ValueError(f'Invalid Edge name: {power!r}')
            ranks.add(rank)
            yield name, power, rank
        if ranks != set(RANKS.values()):
            raise ValueError(f'Incomplete Creed Edge branch {name}')


def extract(args):
    digest, pages = pdf_pages(args, 'imbued')
    if 'lawsofthereckoning' not in key(pages[CONTEXT_LEAF]['text']):
        raise ValueError('Expected Reckoning reference on the packet context page')
    directory = args.catalogues
    profiles = load(directory / 'creature_profiles.json')
    published = load(directory / 'published_powers.json')
    met = load(directory / 'met_powers.json')
    hunter = next(p for p in profiles['profiles'] if p['kind'] == 'Hunter')
    hunter.update(rules_reviewed=True, rules_note=RULES_NOTE,
                  free_trait_costs=dict(attribute=1, ability=1, background=1, primary_power=3,
                                        secondary_power=0, specialization=1, permanent_resource=3),
                  starting_resources=dict(Willpower=3, Conviction=3, Mercy=1, Vision=1, Zeal=1),
                  secondary_power_tier_costs=hunter['power_tier_costs'].copy(),
                  cost_source=dict(book='OWbN Hunter Imbued Packet (2021)',
                                   edition=EDITION, system='Local MET Imbued baseline',
                                   printed_page=1, pdf_page_1based=4,
                                   original_pdf_sha256=digest))
    hunter['trait_xp_costs']['Zeal'] = hunter['trait_xp_costs']['Mercy']
    published['books'] = [b for b in published['books'] if b['id'] != MENU_BOOK]
    published['entries'] = [r for r in published['entries'] if r['book_id'] != MENU_BOOK]
    new_book = dict(id=MENU_BOOK, title='Grapevine Creed Edge menu (2021 Imbued packet context)',
                    edition=EDITION, template_scope='Hunter', pdf_sha256=digest, coverage='partial',
                    notes=['Creed Edge names and ranks come from the legacy menu. The PDF coordinate '
                           'identifies the Imbued packet reference to #5037. Individual MET book '
                           'page citations are pending.'])
    published['books'].append(new_book)
    met['entries'] = [r for r in met['entries'] if not r['stable_id'].startswith((MENU_BOOK, PACKET_BOOK))]
    seen = set()

    def add_met(row, target, authority=AUTHORITY):
        if row['id'] in seen:
            raise ValueError(f"Duplicate Imbued Edge {row['id']}")
        seen.add(row['id'])
        definition = dict(
            stable_id=row['id'], template_scope='Hunter', target_list=target, family=row['discipline'],
            branch=None, name=row['name'], tier=NATIVE_TIERS[row['level'] - 1],
            purchase_family='Other', profile='ConfiguredCreature', printed_page=row['printed_page'],
            pdf_page_1based=row['pdf_page'], original_pdf_sha256=digest, source_reviewed=True,
            prerequisite_note=RULES_NOTE, packet_authority=authority,
            source_level=row['level'], published_source_id=row['id'])
        if 'tier_mapping_basis' in row:
            definition['tier_mapping_basis'] = row['tier_mapping_basis']
        met['entries'].append(definition)

    core_count = 0
    for family, name, rank in creed_edges(args.menu):
        row = dict(id=f'{MENU_BOOK}.{slug(family)}.{slug(name)}.{rank}', book_id=MENU_BOOK,
                   name=name, discipline=family, path=None, kind='edge', level=rank,
                   printed_page=CONTEXT_LEAF - 3, pdf_page=CONTEXT_LEAF, packet_authority=AUTHORITY,
                   evidence='legacy_menu', tier_mapping_basis=BASIS)
        published['entries'].append(row)
        add_met(row, 'EdgeList')
        core_count += 1

    indexed = {row['id']: row for row in published['entries'] if row['book_id'] == PACKET_BOOK}
    for family, name, rank, page, *_ in exact_rows(pages, IMBUED_EDGES):
        row = indexed[f'{PACKET_BOOK}.{slug(family)}.{slug(name)}.{rank}']
        if row['pdf_page'] != page:
            raise ValueError(f'Unexpected packet page for {name}')
        add_met(row, 'PowerList', authority=row['packet_authority'])

    if args.write:
        for name, content in [('creature_profiles', profiles), ('published_powers', published),
                              ('met_powers', met)]:
            write(directory / f'{name}.json', content)
    print(f'{core_count} Creed Edges, {len(seen) - core_count} packet Edges')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('cache', 'menu', 'locators', 'inventory', 'catalogues'):
        parser.add_argument(f'--{name}', required=True, type=Path)
    parser.add_argument('--pdf-root', type=Path)
    parser.add_argument('--write', action='store_true')
    extract(parser.parse_args())
