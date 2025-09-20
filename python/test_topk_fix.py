#!/usr/bin/env python3
"""
Test für TopK Duplikat-Fix mit vielen wiederholten Expressions
"""

from weighted_triemap_client import WeightedTriemapClient

def test_topk_no_duplicates():
    """
    Teste dass TopK keine Duplikate hat, auch bei vielen wiederholten Expressions
    """
    print("=== TopK Duplikat-Fix Test ===")
    
    client = WeightedTriemapClient()
    
    # Erstelle neue Map
    print("\n1. Erstelle 'test_no_dups'...")
    client.wt_create("test_no_dups", k=10)
    
    # Füge viele wiederholte Expressions hinzu
    print("\n2. Füge wiederholte Expressions hinzu...")
    expressions = [
        ("(+ 1 2)", 1.0),   # 1. Mal
        ("(+ 1 2)", 2.0),   # 2. Mal → Total: 3.0
        ("(+ 1 2)", 3.0),   # 3. Mal → Total: 6.0
        ("(+ 1 2)", 4.0),   # 4. Mal → Total: 10.0
        ("(+ 1 2)", 5.0),   # 5. Mal → Total: 15.0
        ("(* 2 3)", 10.0),  # Neue Expression
        ("(+ 1 2)", 1.0),   # 6. Mal → Total: 16.0
        ("(- 5 3)", 3.0),   # Neue Expression
        ("(+ 1 2)", 2.0),   # 7. Mal → Total: 18.0
    ]
    
    for expr, value in expressions:
        result = client.wt_add_sexpr("test_no_dups", expr, value)
        print(f"  Added '{expr}' with value {value}")
    
    # Hole TopK und prüfe auf Duplikate
    print("\n3. TopK Elemente (sollte keine Duplikate haben):")
    topk = client.wt_get_topk("test_no_dups")
    
    seen_paths = set()
    duplicates_found = False
    
    for i, entry in enumerate(topk.data):
        path = entry['path']
        if path in seen_paths:
            print(f"  ❌ DUPLIKAT GEFUNDEN: {path}")
            duplicates_found = True
        else:
            seen_paths.add(path)
            
        print(f"  {i+1}. Path: {path}")
        print(f"     Local Count: {entry['local_count']}")
        print(f"     Compress Gain Sum: {entry['compress_gain_sum']}")
        print(f"     Composite Score: {entry['composite_score']}")
        print()
    
    if not duplicates_found:
        print("✅ Kein Duplikat gefunden - TopK Fix funktioniert!")
    else:
        print("❌ Duplikate gefunden - TopK Fix funktioniert NICHT!")
    
    # Prüfe spezifische Werte für (+ 1 2)
    print("4. Detaillierte Prüfung für '(+ 1 2)':")
    weight = client.wt_get_weight("test_no_dups", "(+ 1 2)")
    print(f"  Local Count: {weight.data['local_count']} (erwartet: 7)")
    print(f"  Compress Gain Sum: {weight.data['compress_gain_sum']} (erwartet: 18.0)")
    print(f"  Composite Weight: {weight.data['composite_weight']} (erwartet: 25.0)")
    
    # Validierung
    expected_local = 7
    expected_gain = 18.0
    expected_composite = 25.0
    
    if (weight.data['local_count'] == expected_local and 
        abs(weight.data['compress_gain_sum'] - expected_gain) < 0.001 and
        abs(weight.data['composite_weight'] - expected_composite) < 0.001):
        print("✅ Alle Werte sind korrekt!")
    else:
        print("❌ Einige Werte sind falsch!")

if __name__ == "__main__":
    test_topk_no_duplicates()
