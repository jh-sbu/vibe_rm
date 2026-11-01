; magic: the recheck interval read by the probe's gain (autotest21 read
; HasMagicEffect, which reports a failing condition's effect). Holds are in
; combat.psc (they need the warehouse).

; The +3 OneHanded the probe on GetHealthPercentage >= 0.9 gives a.
Bool Function HighHolds(Actor a, Float base)
    return a.GetActorValue("OneHanded") - base > 2.5
EndFunction

String Function TestRecheck()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.SetDontMove(true)
    a.ForceActorValue("HealRate", 0.0)
    Utility.Wait(0.5)
    Float base = a.GetActorValue("OneHanded")
    a.AddSpell(PS_HealthHigh, false)
    Utility.Wait(2.0)
    String r = "probe (health >= 90%) at full: " + HighHolds(a, base)
    Int i = 0
    While i < 3
        a.DamageActorValue("Health", a.GetActorValue("Health") * 0.5)
        Float t0 = Now()
        While HighHolds(a, base) && Now() - t0 < 20.0
            Utility.Wait(0.05)
        EndWhile
        r += "; health halved: gone after " + Num(Now() - t0) + " s"
        a.RestoreActorValue("Health", 1000.0)
        t0 = Now()
        While !HighHolds(a, base) && Now() - t0 < 20.0
            Utility.Wait(0.05)
        EndWhile
        r += ", restored: back after " + Num(Now() - t0) + " s"
        Utility.Wait(0.37)
        i += 1
    EndWhile
    a.RemoveSpell(PS_HealthHigh)
    Gone(a)
    return r + "\n"
EndFunction
