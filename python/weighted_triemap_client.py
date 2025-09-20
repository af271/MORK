#!/usr/bin/env python3
"""
WeightedTriemap Client - Erweiterung des MORK Client für WeightedTriemap Funktionalität
"""

import json
from urllib.parse import quote
from client import MORK

class WeightedTriemapClient(MORK):
    """
    Erweiterung des MORK Clients speziell für WeightedTriemap Operationen
    """
    
    class WTCreateRequest(MORK.Request):
        """Request to create a new WeightedTriemap"""
        def __init__(self, map_name, k=10):
            self.map_name = map_name
            self.k = k
            super().__init__("get", f"/wt_create/{quote(map_name)}" + (f"?k={k}" if k != 10 else ""))
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = self.response.text
                
    class WTAddSExprRequest(MORK.Request):
        """Request to add S-expression to WeightedTriemap"""
        def __init__(self, map_name, sexpr, value, extract_subtrees=False):
            self.map_name = map_name
            self.sexpr = sexpr
            self.value = value
            self.extract_subtrees = extract_subtrees
            url = f"/wt_add_sexpr/{quote(map_name)}/{quote(sexpr)}/{value}"
            if extract_subtrees:
                url += "?extract_subtrees=true"
            super().__init__("get", url)
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = self.response.text
                
    class WTAddMorkRequest(MORK.Request):
        """Request to add MORK expression to WeightedTriemap"""
        def __init__(self, map_name, mork_expr, value):
            self.map_name = map_name
            self.mork_expr = mork_expr
            self.value = value
            super().__init__("get", f"/wt_add_mork/{quote(map_name)}/{quote(mork_expr)}/{value}")
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = self.response.text
                
    class WTGetTopKRequest(MORK.Request):
        """Request to get TopK from WeightedTriemap"""
        def __init__(self, map_name):
            self.map_name = map_name
            super().__init__("get", f"/wt_topk/{quote(map_name)}")
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = json.loads(self.response.text)
                
    class WTGetWeightRequest(MORK.Request):
        """Request to get weight of specific expression"""
        def __init__(self, map_name, sexpr):
            self.map_name = map_name
            self.sexpr = sexpr
            super().__init__("get", f"/wt_weight/{quote(map_name)}/{quote(sexpr)}")
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = json.loads(self.response.text)
                
    class WTStatsRequest(MORK.Request):
        """Request to get WeightedTriemap statistics"""
        def __init__(self, map_name):
            self.map_name = map_name
            super().__init__("get", f"/wt_stats/{quote(map_name)}")
            
        def dispatch(self, server):
            super().dispatch(server)
            if self.response and self.response.status_code == 200:
                self.data = json.loads(self.response.text)

    # High-level WeightedTriemap methods
    def wt_create(self, map_name, k=10):
        """
        Erstelle eine neue WeightedTriemap
        
        Args:
            map_name (str): Name der WeightedTriemap
            k (int): Anzahl der TopK Elemente (default: 10)
            
        Returns:
            WTCreateRequest: Request object mit .data für Response
        """
        cmd = self.WTCreateRequest(map_name, k)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd
        
    def wt_add_sexpr(self, map_name, sexpr, value=1.0, extract_subtrees=False):
        """
        Füge S-Expression zur WeightedTriemap hinzu
        
        Args:
            map_name (str): Name der WeightedTriemap
            sexpr (str): S-Expression als String
            value (float): Gewichtswert (wird zu compress_gain_sum addiert)
            extract_subtrees (bool): Ob Subtrees extrahiert werden sollen
            
        Returns:
            WTAddSExprRequest: Request object mit .data für Response
        """
        cmd = self.WTAddSExprRequest(map_name, sexpr, value, extract_subtrees)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd
        
    def wt_add_mork(self, map_name, mork_expr, value=1.0):
        """
        Füge MORK Expression zur WeightedTriemap hinzu
        
        Args:
            map_name (str): Name der WeightedTriemap
            mork_expr (str): MORK Expression als String
            value (float): Gewichtswert
            
        Returns:
            WTAddMorkRequest: Request object mit .data für Response
        """
        cmd = self.WTAddMorkRequest(map_name, mork_expr, value)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd
        
    def wt_get_topk(self, map_name):
        """
        Hole TopK Elemente aus WeightedTriemap
        
        Args:
            map_name (str): Name der WeightedTriemap
            
        Returns:
            WTGetTopKRequest: Request object mit .data als JSON dict
        """
        cmd = self.WTGetTopKRequest(map_name)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd
        
    def wt_get_weight(self, map_name, sexpr):
        """
        Hole Gewicht einer spezifischen Expression
        
        Args:
            map_name (str): Name der WeightedTriemap
            sexpr (str): S-Expression als String
            
        Returns:
            WTGetWeightRequest: Request object mit .data als JSON dict
        """
        cmd = self.WTGetWeightRequest(map_name, sexpr)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd
        
    def wt_get_stats(self, map_name):
        """
        Hole Statistiken der WeightedTriemap
        
        Args:
            map_name (str): Name der WeightedTriemap
            
        Returns:
            WTStatsRequest: Request object mit .data als JSON dict
        """
        cmd = self.WTStatsRequest(map_name)
        self.history.append(cmd)
        cmd.dispatch(self)
        return cmd


def demo_weighted_triemap():
    """
    Demo der WeightedTriemap Funktionalität
    """
    print("=== WeightedTriemap Demo ===")
    
    # Verbinde zum MORK Server
    client = WeightedTriemapClient()
    
    # Erstelle eine neue WeightedTriemap
    print("\n1. Erstelle WeightedTriemap 'demo_map'...")
    result = client.wt_create("demo_map", k=5)
    print(f"Response: {result.data}")
    
    # Füge S-Expressions hinzu
    print("\n2. Füge S-Expressions hinzu...")
    expressions = [
        ("(+ 1 2)", 5.0),
        ("(* 3 4)", 8.0),
        ("(+ 1 2)", 3.0),  # Duplikat - erhöht local_count
        ("(- 10 5)", 2.0),
        ("(+ 1 2)", 1.0),  # Noch ein Duplikat
    ]
    
    for expr, value in expressions:
        result = client.wt_add_sexpr("demo_map", expr, value)
        print(f"Added '{expr}' with value {value}: {result.data}")
    
    # Hole TopK
    print("\n3. TopK Elemente:")
    topk = client.wt_get_topk("demo_map")
    for i, entry in enumerate(topk.data):
        print(f"  {i+1}. Path: {entry['path']}")
        print(f"     Local Count: {entry['local_count']}")
        print(f"     Subtree Count: {entry['subtree_count']}")
        print(f"     Compress Gain Sum: {entry['compress_gain_sum']}")
        print(f"     Composite Score: {entry['composite_score']}")
        print()
    
    # Hole Gewicht einer spezifischen Expression
    print("4. Gewicht von '(+ 1 2)':")
    weight = client.wt_get_weight("demo_map", "(+ 1 2)")
    print(f"  Local Count: {weight.data['local_count']}")
    print(f"  Subtree Count: {weight.data['subtree_count']}")
    print(f"  Compress Gain Sum: {weight.data['compress_gain_sum']}")
    print(f"  Composite Weight: {weight.data['composite_weight']}")
    
    # Hole Statistiken
    print("\n5. Map Statistiken:")
    stats = client.wt_get_stats("demo_map")
    print(f"  Name: {stats.data['name']}")
    print(f"  Total TopK Entries: {stats.data['total_topk_entries']}")
    print(f"  Highest Local Count: {stats.data['highest_local_count']}")
    print(f"  Highest Composite Score: {stats.data['highest_composite_score']}")


if __name__ == "__main__":
    demo_weighted_triemap()
