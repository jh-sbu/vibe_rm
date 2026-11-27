; autotest29, in the test pens (VrmAt21Pens): whether NPCs catch a
; creature's disease. Nine pens, each with a held, unaggressive NPC and a
; bear, whose attacks carry Bone Break Fever (6% a blow: its effect's
; GetRandomPercent <= 5).

; Pen i's marker.
ObjectReference Function Pen(Int i)
    If i == 0
        return Pen0
    ElseIf i == 1
        return Pen1
    ElseIf i == 2
        return Pen2
    ElseIf i == 3
        return Pen3
    ElseIf i == 4
        return Pen4
    ElseIf i == 5
        return Pen5
    ElseIf i == 6
        return Pen6
    ElseIf i == 7
        return Pen7
    EndIf
    return Pen8
EndFunction

; An actor of base b placed in pen i, x / y from its centre.
Actor Function SpawnPen(ActorBase b, Int i, Float x, Float y)
    Actor a = Pen(i).PlaceActorAtMe(b, 4, None)
    a.MoveTo(Pen(i), x, y, 0.0, false)
    return a
EndFunction

Function GoPens()
    Player.MoveTo(PenBooth, 0.0, 0.0, 10.0, true)
    Utility.Wait(4.0)
EndFunction

; The battery itself: the player in the booth, and the pens' distances.
String Function TestPensEnter()
    GoPens()
    String r = "in the booth: interior " + Player.IsInInterior() + ", z above the pens " + Num(Player.GetPositionZ() - Pen4.GetPositionZ()) + "\n  distances to the pens:"
    Int i = 0
    While i < 9
        r += " " + Num(Player.GetDistance(Pen(i)))
        i += 1
    EndWhile
    return r + "\n"
EndFunction

String Function TestNpcDisease()
    Actor[] subject = new Actor[9]
    Actor[] bear = new Actor[9]
    Float[] last = new Float[9]
    Int[] hits = new Int[9]
    Float[] spellAt = new Float[9]
    Float[] effectAt = new Float[9]
    Float[] stamina = new Float[9]
    Int i = 0
    While i < 9
        subject[i] = SpawnPen(NakedBase, i, 0.0, -200.0)
        bear[i] = SpawnPen(BearBase, i, 0.0, 250.0)
        i += 1
    EndWhile
    Utility.Wait(2.0)
    i = 0
    While i < 9
        Actor a = subject[i]
        Tough(a)
        Tough(bear[i])
        a.SetActorValue("DiseaseResist", 0.0)
        a.SetActorValue("Aggression", 0.0)
        a.SetDontMove(true)
        a.RemoveSpell(BoneBreak)
        last[i] = Health(a)
        spellAt[i] = -1.0
        effectAt[i] = -1.0
        stamina[i] = a.GetActorValueMax("Stamina")
        i += 1
    EndWhile
    String r = "9 pens, each a level " + subject[0].GetLevel() + " unaggressive NPC (DiseaseResist " + Num(subject[0].GetActorValue("DiseaseResist")) + ", held by SetDontMove, 5000 health) and a level " + bear[0].GetLevel() + " bear 450 off, StartCombat on it; 150 s\n"
    i = 0
    While i < 9
        bear[i].StartCombat(subject[i])
        i += 1
    EndWhile
    Utility.Wait(3.0)
    r += "  at 3 s, bear state/targets its NPC/distance:"
    i = 0
    While i < 9
        r += " " + bear[i].GetCombatState() + "/" + (bear[i].GetCombatTarget() == subject[i]) + "/" + Num(bear[i].GetDistance(subject[i]))
        i += 1
    EndWhile
    r += "\n"
    Float start = Now()
    While Now() - start < 150.0
        Utility.Wait(0.25)
        i = 0
        While i < 9
            Actor s = subject[i]
            Float h = Health(s)
            If h < last[i] - 0.5
                hits[i] = hits[i] + 1
            EndIf
            If h < 4000.0
                s.RestoreActorValue("Health", 5000.0)
                h = Health(s)
            EndIf
            last[i] = h
            If spellAt[i] < 0.0 && s.HasSpell(BoneBreak)
                spellAt[i] = Now() - start
            EndIf
            If effectAt[i] < 0.0 && s.HasMagicEffect(BoneBreakEffect)
                effectAt[i] = Now() - start
            EndIf
            i += 1
        EndWhile
    EndWhile
    i = 0
    While i < 9
        bear[i].StopCombat()
        Gone(bear[i])
        i += 1
    EndWhile
    Utility.Wait(2.0)
    ; Per NPC: health drops seen (a poll may merge blows), when HasSpell and
    ; HasMagicEffect first read true (-1: never), max stamina before and
    ; after (Bone Break Fever drains 25), and HasSpell at the end.
    i = 0
    While i < 9
        Actor c = subject[i]
        r += "  pen " + i + ": " + hits[i] + " drops, HasSpell at " + Num(spellAt[i]) + ", HasMagicEffect at " + Num(effectAt[i]) + ", max stamina " + Num(stamina[i]) + " -> " + Num(c.GetActorValueMax("Stamina")) + ", HasSpell now " + c.HasSpell(BoneBreak) + "\n"
        i += 1
    EndWhile
    i = 0
    While i < 9
        subject[i].RemoveSpell(BoneBreak)
        subject[i].SetDontMove(false)
        Gone(subject[i])
        i += 1
    EndWhile
    return r
EndFunction
