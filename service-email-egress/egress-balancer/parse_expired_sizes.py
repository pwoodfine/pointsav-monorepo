# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

import xml.etree.ElementTree as ET, sys
ns = {'t': 'http://schemas.microsoft.com/exchange/services/2006/types'}
try:
    tree = ET.parse(sys.argv[1])
    for item in tree.findall('.//t:ItemId/..', ns):
        i_id = item.find('t:ItemId', ns).attrib.get('Id', '')
        size_node = item.find('t:Size', ns)
        size_val = size_node.text if size_node is not None else '0'
        print("{}|{}".format(i_id, size_val))
except Exception:
    pass
