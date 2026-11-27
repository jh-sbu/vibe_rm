; autotest30, in the test pens: who a hostile spell's held target fights,
; and Fury's target when its caster is unseen or gone.

; magic (autotest28's open item): a lawful NPC held by SetDontMove and hit
; by a script cast of our fire spell searched (state 2) in SE where a free
; one fought. Held by the hold, or out of reach of its caster? The caster
; is an unaggressive NPC in the pen (held), 100 or 400 off; the target
; held or free, unarmed or with a sword. Pens: 0 held 100, 1 held 400,
; 2 free 100, 3 free 400, all unarmed; 4-7 the same with a sword; 8 held,
; unarmed, 250.
Bool Function HostileHeld(Int i)
    Int k = i % 4
    return i == 8 || k == 0 || k == 1
EndFunction

Float Function HostileDistance(Int i)
    If i == 8
        return 250.0
    ElseIf i % 2 == 0
        return 100.0
    EndIf
    return 400.0
EndFunction

String Function TestHostileHeld()
    Actor[] target = new Actor[9]
    Actor[] caster = new Actor[9]
    String[] reads = new String[9]
    Int i = 0
    While i < 9
        Actor t = SpawnPen(LoggedBase, i, 0.0, -200.0)
        Actor c = SpawnPen(NakedBase, i, 0.0, -200.0 + HostileDistance(i))
        target[i] = t
        caster[i] = c
        t.SetCrimeFaction(CrimeFaction)
        If i >= 4 && i < 8
            t.AddItem(IronSword, 1, true)
            t.EquipItem(IronSword, false, true)
        EndIf
        t.SetDontMove(HostileHeld(i))
        c.SetDontMove(true)
        Tough(t)
        Tough(c)
        i += 1
    EndWhile
    Utility.Wait(2.0)
    CrimeFaction.SetCrimeGold(0)
    i = 0
    While i < 9
        WhFireSpell.Cast(caster[i], target[i])
        i += 1
    EndWhile
    Float t0 = Now()
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        i = 0
        While i < 9
            reads[i] = reads[i] + " " + target[i].GetCombatState() + "/" + Who(target[i], caster[i], None)
            i += 1
        EndWhile
        s += 1
    EndWhile
    String r = "our fire spell cast by script from an NPC at a lawful NPC; target's state/combat target at 2, 4, 6 and 8 s:\n"
    i = 0
    While i < 9
        String held = "free"
        If HostileHeld(i)
            held = "held"
        EndIf
        String arms = "unarmed"
        If i >= 4 && i < 8
            arms = "sword"
        EndIf
        r += "  pen " + i + " (" + held + ", " + arms + ", caster " + Num(target[i].GetDistance(caster[i])) + " off):" + reads[i] + "; drawn " + target[i].IsWeaponDrawn() + "\n"
        i += 1
    EndWhile
    r += "  crime gold " + CrimeFaction.GetCrimeGold() + "\n"
    i = 0
    While i < 9
        target[i].StopCombat()
        caster[i].StopCombat()
        target[i].SetDontMove(false)
        Gone(target[i])
        Gone(caster[i])
        i += 1
    EndWhile
    CrimeFaction.SetCrimeGold(0)
    SweepPens(IronSword)
    return r
EndFunction

; Whom a is fighting: "caster", "other", "player", another's id, or none.
String Function Who(Actor a, Actor caster, Actor other)
    Actor t = a.GetCombatTarget()
    If t == None
        return "none"
    ElseIf t == caster
        return "caster"
    ElseIf other != None && t == other
        return "other"
    ElseIf t == Player
        return "player"
    EndIf
    return Id(t)
EndFunction

; magic (ours sends a frenzied target at its caster, else the nearest): our
; Fury (6) on a level 1 unaggressive NPC. Pens 0-2: cast by the player
; from the booth (unseen, 1100 up and more), another NPC 150 off; 3-5: the
; same with nobody else in the pen; 6-8: cast by an NPC 400 off (held),
; the other NPC 150 off, the caster taken away 6 s in. State/target/
; aggression every 2 s for 16 s.
String Function TestFuryUnseen()
    Actor[] f = new Actor[9]
    Actor[] other = new Actor[9]
    Actor[] caster = new Actor[9]
    String[] reads = new String[9]
    Int i = 0
    While i < 9
        f[i] = SpawnPen(LowLoggedBase, i, 0.0, 0.0)
        If i < 3 || i >= 6
            other[i] = SpawnPen(NakedBase, i, 150.0, 0.0)
            other[i].SetDontMove(true)
            Tough(other[i])
        EndIf
        If i >= 6
            caster[i] = SpawnPen(NakedBase, i, 0.0, -400.0)
            caster[i].SetDontMove(true)
            Tough(caster[i])
        Else
            caster[i] = Player
        EndIf
        Tough(f[i])
        i += 1
    EndWhile
    Utility.Wait(2.0)
    i = 0
    While i < 9
        OurFury.Cast(caster[i], f[i])
        i += 1
    EndWhile
    Int s = 1
    While s <= 8
        Utility.Wait(2.0)
        i = 0
        While i < 9
            reads[i] = reads[i] + " " + f[i].GetCombatState() + "/" + Who(f[i], caster[i], other[i]) + "/" + Num(f[i].GetActorValue("Aggression"))
            If s == 3 && i >= 6
                caster[i].StopCombat()
                Gone(caster[i])
            EndIf
            i += 1
        EndWhile
        s += 1
    EndWhile
    String r = "our Fury (6) on a level 1 NPC; state/target/aggression every 2 s for 16 s:\n"
    i = 0
    While i < 9
        String setup = "the player casting from the booth, another NPC 150 off"
        If i >= 3 && i < 6
            setup = "the player casting from the booth, nobody else"
        ElseIf i >= 6
            setup = "an NPC casting 400 off (gone after the third read), another 150 off"
        EndIf
        String lost = ""
        If other[i] != None
            lost = "; the other lost " + Num(other[i].GetActorValueMax("Health") - Health(other[i]))
        EndIf
        r += "  pen " + i + " (" + setup + "):" + reads[i] + lost + "\n"
        i += 1
    EndWhile
    i = 0
    While i < 9
        f[i].StopCombat()
        Gone(f[i])
        If other[i] != None
            other[i].StopCombat()
            Gone(other[i])
        EndIf
        i += 1
    EndWhile
    return r
EndFunction
