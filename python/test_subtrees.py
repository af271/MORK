#!/usr/bin/env python3
"""
Test für Subtree-Extraktion in WeightedTriemap
"""

from weighted_triemap_client import WeightedTriemapClient

def test_subtree_extraction():
    """
    Teste dass Subtrees korrekt extrahiert werden
    """
    print("=== Subtree Extraction Test ===")
    
    client = WeightedTriemapClient()
    
    # Erstelle neue Map
    print("\n1. Erstelle 'subtree_test'...")
    client.wt_create("subtree_test", k=20)
    
    # Füge komplexe S-Expression mit Subtree-Extraktion hinzu
    complex_expr = "(company (department (team (person (name Alice) (role developer)) (person (name Charlie) (role designer))) (budget 60000)) (location Munich))"
    
    print(f"\n2. Füge komplexe Expression hinzu MIT Subtree-Extraktion...")
    print(f"Expression: {complex_expr}")
    
    result = client.wt_add_sexpr("subtree_test", complex_expr, 10.0, extract_subtrees=True)
    print(f"Response: {result.data}")
    
    # Hole TopK und schaue nach Subtrees
    print("\n3. TopK Elemente (sollte Subtrees enthalten):")
    topk = client.wt_get_topk("subtree_test")
    
    print(f"Anzahl TopK Einträge: {len(topk.data)}")
    
    for i, entry in enumerate(topk.data):
        path = entry['path']
        print(f"  {i+1}. Path: {path}")
        print(f"     Local Count: {entry['local_count']}")
        print(f"     Compress Gain Sum: {entry['compress_gain_sum']}")
        print(f"     Composite Score: {entry['composite_score']}")
        print()
    
    # Teste spezifische Subtrees
    print("4. Teste spezifische Subtrees:")
    subtrees_to_test = [
        "(name Alice)",
        "(role developer)", 
        "(person (name Alice) (role developer))",
        "(team (person (name Alice) (role developer)) (person (name Charlie) (role designer)))",
        "(department (team (person (name Alice) (role developer)) (person (name Charlie) (role designer))) (budget 60000))"
    ]
    
    for subtree in subtrees_to_test:
        result = client.wt_get_weight("subtree_test", subtree)
        if 'found' in result.data and not result.data['found']:
            print(f"  ❌ '{subtree}' NICHT gefunden")
        else:
            print(f"  ✅ '{subtree}' gefunden - Local Count: {result.data.get('local_count', 'N/A')}")

if __name__ == "__main__":
    test_subtree_extraction()
