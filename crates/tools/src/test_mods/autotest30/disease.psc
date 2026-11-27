; autotest30, in the test pens: autotest29's open items on diseases.

; Disease Resist 0 (pens 0-2), 50 (3-5) and 100 (6-8): an NPC as
; autotest29's (held, 5000 health kept up) struck by a bear (Bone Break
; Fever, 6% a blow) for up to 150 s. Once HasMagicEffect shows the disease
; its bear is taken away, and its max stamina is read every second for
; 120 s more: does the 25-point drain stay (rolled once), come and go
; (rolled again), or never come? GetDisease (the probe) on each at the end.
Float Function ResistOf(Int i)
    If i < 3
        return 0.0
    ElseIf i < 6
        return 50.0
    EndIf
    return 100.0
EndFunction

String Function TestDiseaseResist()
    Actor[] subject = new Actor[9]
    Actor[] bear = new Actor[9]
    Float[] stamina = new Float[9]
    Float[] caughtAt = new Float[9]
    Int[] polls = new Int[9]
    Int[] drained = new Int[9]
    Int[] flips = new Int[9]
    Int[] was = new Int[9]
    Float[] low = new Float[9]
    Float[] high = new Float[9]
    Float[] lastPoll = new Float[9]
    Int i = 0
    While i < 9
        subject[i] = SpawnPen(LoggedBase, i, 0.0, -200.0)
        bear[i] = SpawnPen(BearBase, i, 0.0, 250.0)
        i += 1
    EndWhile
    Utility.Wait(2.0)
    i = 0
    While i < 9
        Actor a = subject[i]
        Tough(a)
        Tough(bear[i])
        a.SetActorValue("DiseaseResist", ResistOf(i))
        a.SetActorValue("Aggression", 0.0)
        a.SetDontMove(true)
        a.RemoveSpell(BoneBreak)
        stamina[i] = a.GetActorValueMax("Stamina")
        caughtAt[i] = -1.0
        low[i] = 10000.0
        high[i] = -1.0
        (a as VrmAt21ActorLog).Watch(BoneBreak, BoneBreakEffect, stamina[i] - 20.0)
        i += 1
    EndWhile
    i = 0
    While i < 9
        bear[i].StartCombat(subject[i])
        i += 1
    EndWhile
    Float start = Now()
    Bool going = true
    While going
        Utility.Wait(0.25)
        Float now = Now() - start
        going = false
        i = 0
        While i < 9
            Actor s = subject[i]
            If Health(s) < 4000.0
                s.RestoreActorValue("Health", 5000.0)
            EndIf
            If caughtAt[i] < 0.0
                If s.HasMagicEffect(BoneBreakEffect)
                    caughtAt[i] = now
                    lastPoll[i] = now
                    bear[i].StopCombat()
                    Gone(bear[i])
                    bear[i] = None
                ElseIf now < 150.0
                    going = true
                ElseIf bear[i] != None
                    ; Not caught in 150 s: its bear goes.
                    bear[i].StopCombat()
                    Gone(bear[i])
                    bear[i] = None
                EndIf
            EndIf
            If caughtAt[i] >= 0.0 && now - caughtAt[i] < 120.0
                going = true
                If now - lastPoll[i] >= 1.0
                    lastPoll[i] = now
                    Float m = s.GetActorValueMax("Stamina")
                    Int d = 0
                    If m < stamina[i] - 20.0
                        d = 1
                    EndIf
                    If polls[i] > 0 && d != was[i]
                        flips[i] = flips[i] + 1
                    EndIf
                    was[i] = d
                    polls[i] = polls[i] + 1
                    drained[i] = drained[i] + d
                    If m < low[i]
                        low[i] = m
                    EndIf
                    If m > high[i]
                        high[i] = m
                    EndIf
                EndIf
            EndIf
            i += 1
        EndWhile
        If now > 300.0
            going = false
        EndIf
    EndWhile
    i = 0
    While i < 9
        If bear[i] != None
            bear[i].StopCombat()
            Gone(bear[i])
        EndIf
        i += 1
    EndWhile
    Utility.Wait(2.0)
    String r = "Disease Resist 0 (pens 0-2), 50 (3-5), 100 (6-8): an NPC struck by a bear until HasMagicEffect shows Bone Break Fever (150 s at most), then its bear gone and its max stamina (" + Num(stamina[0]) + ") read each second for 120 s:\n"
    i = 0
    While i < 9
        Actor s = subject[i]
        r += "  pen " + i + " (resist " + Num(s.GetActorValue("DiseaseResist")) + "): " + (s as VrmAt21ActorLog).Counts()
        If caughtAt[i] >= 0.0
            r += "; caught at " + Num(caughtAt[i]) + " s; then " + drained[i] + " of " + polls[i] + " reads drained, " + flips[i] + " changes, max stamina " + Num(low[i]) + " to " + Num(high[i]) + ", now " + Num(s.GetActorValueMax("Stamina")) + "; GetDisease " + Probe(s, PS_Disease, PE_Disease)
        EndIf
        r += "\n"
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
