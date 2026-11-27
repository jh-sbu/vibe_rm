; autotest30, in the test pens: entering them, NPCs' regeneration and
; explosions' knockdowns of held, free and restrained NPCs.

String Function TestPensEnter()
    GoPens()
    String r = "in the booth: interior " + Player.IsInInterior() + ", z above the pens " + Num(Player.GetPositionZ() - Pen4.GetPositionZ()) + "\n"
    return r
EndFunction

; regen (test-mod regen's open item): whether NPCs' rate multipliers count.
; Ours ignores them; NPCs' HealRate reads 0 (autotest5), so the health pens
; set it to 0.7, the races' value. Pens 0-2: health at HealRateMult 100, 0,
; 300; 3-5: stamina at StaminaRateMult 100, 0, 300; 6-8: magicka at
; MagickaRateMult 100, 0, 300. Each value dropped (health to half, the
; others to a tenth), then read at 1.5 s and 5.5 s (at 300, stamina
; would be full again within 10 s): percent of the most a second.
String Function RegenValue(Int i)
    If i < 3
        return "Health"
    ElseIf i < 6
        return "Stamina"
    EndIf
    return "Magicka"
EndFunction

String Function RegenMult(Int i)
    If i < 3
        return "HealRateMult"
    ElseIf i < 6
        return "StaminaRateMult"
    EndIf
    return "MagickaRateMult"
EndFunction

Float Function RegenMultValue(Int i)
    Int k = i % 3
    If k == 0
        return 100.0
    ElseIf k == 1
        return 0.0
    EndIf
    return 300.0
EndFunction

String Function RegenRate(Int i)
    If i < 3
        return "HealRate"
    ElseIf i < 6
        return "StaminaRate"
    EndIf
    return "MagickaRate"
EndFunction

String Function TestNpcRegen()
    Actor[] a = new Actor[9]
    Float[] first = new Float[9]
    Int i = 0
    While i < 9
        a[i] = SpawnPen(LoggedBase, i, 0.0, 0.0)
        a[i].SetDontMove(true)
        i += 1
    EndWhile
    Utility.Wait(2.0)
    String r = "a level " + a[0].GetLevel() + " NPC in each pen, held, out of combat; value: rate, multiplier, % of the most a second from 1.5 s to 5.5 s after the drop:\n"
    i = 0
    While i < 9
        Actor n = a[i]
        String v = RegenValue(i)
        If i < 3
            n.SetActorValue("HealRate", 0.7)
        EndIf
        n.SetActorValue(RegenMult(i), RegenMultValue(i))
        n.RestoreActorValue(v, 10000.0)
        Float most = n.GetActorValueMax(v)
        If i < 3
            n.DamageActorValue(v, most * 0.5)
        Else
            n.DamageActorValue(v, most * 0.9)
        EndIf
        i += 1
    EndWhile
    Utility.Wait(1.5)
    i = 0
    While i < 9
        first[i] = a[i].GetActorValue(RegenValue(i))
        i += 1
    EndWhile
    Float t0 = Now()
    Utility.Wait(4.0)
    Float t = Now() - t0
    i = 0
    While i < 9
        Actor n = a[i]
        String v = RegenValue(i)
        Float most = n.GetActorValueMax(v)
        Float rate = 0.0
        If most > 0.0
            rate = (n.GetActorValue(v) - first[i]) / most * 100.0 / t
        EndIf
        r += "  pen " + i + ": " + v + " (most " + Num(most) + "): " + RegenRate(i) + " " + Num(n.GetActorValue(RegenRate(i))) + ", " + RegenMult(i) + " " + Num(n.GetActorValue(RegenMult(i))) + ": " + Num(rate) + "% a second\n"
        i += 1
    EndWhile
    i = 0
    While i < 9
        Gone(a[i])
        i += 1
    EndWhile
    return r
EndFunction

; explosions (test-mod explosions' open item): whether SetDontMove (or
; SetRestrained) keeps an NPC from a knockdown. An NPC at each pen's centre
; (1000 more health); "Always" (fakeForceBallLite1024) 50 west of it, 30 up,
; in all nine at once. Pens 0-2 held by SetDontMove, 3-5 free, 6-8
; restrained. Knocked: the probe (GetKnockedStateEnum >= 1), added before
; and read every 0.25 s for 5 s; moved: the farthest from where it stood.
String Function TestKnockdownHolds()
    Actor[] a = new Actor[9]
    Float[] base = new Float[9]
    Float[] x0 = new Float[9]
    Float[] y0 = new Float[9]
    Float[] knockedAt = new Float[9]
    Float[] moved = new Float[9]
    ObjectReference[] spot = new ObjectReference[9]
    Int i = 0
    While i < 9
        Actor n = SpawnPen(LoggedBase, i, 0.0, 0.0)
        a[i] = n
        If i < 3
            n.SetDontMove(true)
        ElseIf i >= 6
            n.SetRestrained(true)
        EndIf
        n.ModActorValue("Health", 1000.0)
        n.RestoreActorValue("Health", 2000.0)
        i += 1
    EndWhile
    Utility.Wait(2.0)
    i = 0
    While i < 9
        base[i] = a[i].GetActorValue("OneHanded")
        a[i].AddSpell(PS_Knocked, false)
        spot[i] = PutPen(XMarker, i, -50.0, 0.0, 30.0)
        knockedAt[i] = -1.0
        i += 1
    EndWhile
    Utility.Wait(2.5)
    i = 0
    While i < 9
        x0[i] = a[i].GetPositionX()
        y0[i] = a[i].GetPositionY()
        i += 1
    EndWhile
    String r = "Always (fakeForceBallLite1024) 50 west of an NPC in each pen; before it, knocked:"
    i = 0
    While i < 9
        r += " " + Num(a[i].GetActorValue("OneHanded") - base[i])
        i += 1
    EndWhile
    r += " (probe gain; 3 is knocked)\n"
    Float t0 = Now()
    i = 0
    While i < 9
        spot[i].PlaceAtMe(ExplAlways, 1, false, false)
        i += 1
    EndWhile
    While Now() - t0 < 5.0
        Utility.Wait(0.25)
        i = 0
        While i < 9
            Actor n = a[i]
            If knockedAt[i] < 0.0 && n.GetActorValue("OneHanded") - base[i] > 2.5
                knockedAt[i] = Now() - t0
            EndIf
            Float dx = n.GetPositionX() - x0[i]
            Float dy = n.GetPositionY() - y0[i]
            Float d = Math.Sqrt(dx * dx + dy * dy)
            If d > moved[i]
                moved[i] = d
            EndIf
            i += 1
        EndWhile
    EndWhile
    i = 0
    While i < 9
        String hold = "free"
        If i < 3
            hold = "SetDontMove"
        ElseIf i >= 6
            hold = "SetRestrained"
        EndIf
        r += "  pen " + i + " (" + hold + "): knocked from " + Num(knockedAt[i]) + " s (-1: never), moved up to " + Num(moved[i]) + ", health lost " + Num(a[i].GetActorValueMax("Health") - Health(a[i])) + "\n"
        i += 1
    EndWhile
    i = 0
    While i < 9
        a[i].RemoveSpell(PS_Knocked)
        a[i].SetDontMove(false)
        a[i].SetRestrained(false)
        Gone(a[i])
        Gone(spot[i])
        i += 1
    EndWhile
    return r
EndFunction
