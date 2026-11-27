; autotest30, in the test pens: a disarmed fighter's next weapon, and the
; Vigilant's healing by attacker and health.

; disarm (known_ambiguities/disarm.md): autotest24's disarmed NPC carrying
; a dagger fought on with nothing in hand for at least 4 s. A level 1
; fighter with a sword fights another; our Disarm (12) is cast at it by
; its opponent (by script; the player in the booth may not reach it) once
; it swings (again if the sword stays, three times at
; most), then watched for 60 s: when it first holds a weapon again, and
; which. Pens 0-2: a dagger carried; 3-5: none; 6-8: a dagger, the
; opponent taken away as it is disarmed.
String Function TestDisarmDagger()
    Actor[] a = new Actor[9]
    Actor[] o = new Actor[9]
    Int[] phase = new Int[9]
    Int[] tries = new Int[9]
    Float[] castAt = new Float[9]
    Float[] disarmedAt = new Float[9]
    Float[] armedAt = new Float[9]
    String[] armedWith = new String[9]
    String[] states = new String[9]
    Int i = 0
    While i < 9
        a[i] = SpawnPen(LowFighterLoggedBase, i, 0.0, -100.0)
        o[i] = SpawnPen(LowFighterBase, i, 0.0, 150.0)
        If i < 3 || i >= 6
            a[i].AddItem(Dagger, 1, true)
        EndIf
        Tough(a[i])
        Tough(o[i])
        armedAt[i] = -1.0
        disarmedAt[i] = -1.0
        i += 1
    EndWhile
    Utility.Wait(2.0)
    i = 0
    While i < 9
        Fight(a[i], o[i])
        i += 1
    EndWhile
    ; phase 0: waiting for a swing; 1: cast, the sword not yet seen to go;
    ; 2: disarmed, watched; 3: done (watched 60 s, or three casts missed).
    Float t0 = Now()
    Int done = 0
    While done < 9 && Now() - t0 < 200.0
        done = 0
        i = 0
        While i < 9
            Actor n = a[i]
            Float now = Now()
            If phase[i] == 0
                If n.GetAnimationVariableBool("IsAttacking")
                    OurDisarm.Cast(o[i], n)
                    castAt[i] = now
                    tries[i] = tries[i] + 1
                    phase[i] = 1
                EndIf
            ElseIf phase[i] == 1
                If n.GetEquippedWeapon(false) == None
                    disarmedAt[i] = now
                    phase[i] = 2
                    If i >= 6
                        o[i].StopCombat()
                        n.StopCombat()
                        Gone(o[i])
                    EndIf
                ElseIf now - castAt[i] > 1.5
                    If tries[i] >= 3
                        phase[i] = 3
                    Else
                        phase[i] = 0
                    EndIf
                EndIf
            ElseIf phase[i] == 2
                Weapon w = n.GetEquippedWeapon(false)
                If armedAt[i] < 0.0 && w != None
                    armedAt[i] = now - disarmedAt[i]
                    If w == Dagger
                        armedWith[i] = "the dagger"
                    ElseIf w == IronSword
                        armedWith[i] = "the sword"
                    Else
                        armedWith[i] = Id(w)
                    EndIf
                EndIf
                If now - disarmedAt[i] >= 60.0
                    phase[i] = 3
                EndIf
            EndIf
            If phase[i] == 3
                done += 1
            EndIf
            i += 1
        EndWhile
        Utility.Wait(0.05)
    EndWhile
    String r = "a level 1 fighter with a sword, disarmed mid-swing by our Disarm from its opponent; after it, for 60 s:\n"
    i = 0
    While i < 9
        String setup = "a dagger carried"
        If i >= 3 && i < 6
            setup = "no dagger"
        ElseIf i >= 6
            setup = "a dagger, the opponent gone"
        EndIf
        r += "  pen " + i + " (" + setup + "): " + tries[i] + " casts, "
        If disarmedAt[i] < 0.0
            r += "never disarmed\n"
        Else
            Actor n = a[i]
            Bool swordLies = Game.FindClosestReferenceOfTypeFromRef(IronSword, Pen(i), 700.0) != None
            r += "armed again after " + Num(armedAt[i]) + " s (-1: not in 60 s) with " + armedWith[i] + "; at the end holds " + Id(n.GetEquippedWeapon(false)) + ", state " + n.GetCombatState() + ", a sword lying in the pen " + swordLies + "\n"
        EndIf
        i += 1
    EndWhile
    i = 0
    While i < 9
        a[i].StopCombat()
        Gone(a[i])
        If i < 6
            o[i].StopCombat()
            Gone(o[i])
        EndIf
        i += 1
    EndWhile
    SweepPens(IronSword)
    SweepPens(Dagger)
    return r
EndFunction

; npc-healing (autotest28's open items): the Vigilant (600 health, HealRate
; 0, full magicka) healed at about 0.16 and below under fire, never at
; 0.218 against melee. Against a level 1 fighter (melee) or a fire warlock
; (fire), from: pens 0-2 melee 0.20, 0.18, 0.15; 3-5 fire 0.20, 0.18,
; 0.15; 6 melee 0.10; 7 fire 0.10; 8 fire 0.15 with no magicka (a potion?).
; Health fraction/magicka every 4 s for 28 s; its casts and bleedouts.
Bool Function HealFire(Int i)
    return (i >= 3 && i < 6) || i == 7 || i == 8
EndFunction

Float Function HealFrom(Int i)
    If i == 6 || i == 7
        return 0.10
    ElseIf i == 8
        return 0.15
    EndIf
    Int k = i % 3
    If k == 0
        return 0.20
    ElseIf k == 1
        return 0.18
    EndIf
    return 0.15
EndFunction

String Function TestHealing()
    Actor[] v = new Actor[9]
    Actor[] at = new Actor[9]
    String[] reads = new String[9]
    Int i = 0
    While i < 9
        v[i] = SpawnPen(VigilantLogBase, i, 0.0, -150.0)
        If HealFire(i)
            at[i] = SpawnPen(WarlockLogBase, i, 0.0, 250.0)
        Else
            at[i] = SpawnPen(LowFighterBase, i, 0.0, 150.0)
        EndIf
        Tough(at[i])
        i += 1
    EndWhile
    Utility.Wait(1.5)
    i = 0
    While i < 9
        Actor h = v[i]
        h.SetActorValue("Health", 600.0)
        h.RestoreActorValue("Health", 10000.0)
        h.ForceActorValue("HealRate", 0.0)
        h.RestoreActorValue("Magicka", 1000.0)
        If i == 8
            h.DamageActorValue("Magicka", h.GetActorValue("Magicka"))
        EndIf
        i += 1
    EndWhile
    Utility.Wait(0.5)
    i = 0
    While i < 9
        Actor h = v[i]
        h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - HealFrom(i)))
        (h as VrmAt21ActorLog).MarkCasts()
        reads[i] = " 0:" + Num(h.GetActorValuePercentage("Health")) + "/" + Num(h.GetActorValue("Magicka"))
        Fight(at[i], h)
        i += 1
    EndWhile
    Float t0 = Now()
    Int s = 1
    While s <= 7
        Utility.Wait(4.0)
        i = 0
        While i < 9
            Actor h = v[i]
            reads[i] = reads[i] + " " + (s * 4) + ":" + Num(h.GetActorValuePercentage("Health")) + "/" + Num(h.GetActorValue("Magicka"))
            i += 1
        EndWhile
        s += 1
    EndWhile
    String r = "a Vigilant (600 health) in each pen; health fraction/magicka every 4 s, then its casts (seconds in):\n"
    i = 0
    While i < 9
        String by = "melee"
        If HealFire(i)
            by = "fire"
        EndIf
        r += "  pen " + i + " (" + by + " from " + Num(HealFrom(i)) + "):" + reads[i] + "; " + (v[i] as VrmAt21ActorLog).TakeCasts() + "\n"
        i += 1
    EndWhile
    i = 0
    While i < 9
        v[i].StopCombat()
        at[i].StopCombat()
        Gone(v[i])
        Gone(at[i])
        i += 1
    EndWhile
    SweepPens(IronSword)
    return r
EndFunction
