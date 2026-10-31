; Magic in the test cell, cast by script between held actors: absorb,
; resistance, value holds and level caps, cloaks, ethereal, Slow Time, soul
; trap, worn enchantments, the condition recheck interval.

String Function TestAbsorbMagicka()
    Actor c = Spawn(NakedBase, 0.0, 300.0)
    Actor t = Spawn(NakedBase, 0.0, 500.0)
    c.SetDontMove(true)
    t.SetDontMove(true)
    c.ForceActorValue("MagickaRate", 0.0)
    t.ForceActorValue("MagickaRate", 0.0)
    Utility.Wait(1.0)
    c.DamageActorValue("Magicka", 40.0)
    Float cm = c.GetActorValue("Magicka")
    Float tm = t.GetActorValue("Magicka")
    AbsorbMagickaSpell.Cast(c, t)
    Utility.Wait(2.0)
    String r = "a 20 point absorb magicka (target actor), caster down 40: target lost " + Num(tm - t.GetActorValue("Magicka")) + ", caster gained " + Num(c.GetActorValue("Magicka") - cm) + "\n"
    Gone(c)
    Gone(t)
    return r
EndFunction

; How soon a lasting effect's condition is looked at again: a probe needing
; health at 90% or more, health dropped and raised under it.
String Function TestRecheck()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.SetDontMove(true)
    a.ForceActorValue("HealRate", 0.0)
    Utility.Wait(0.5)
    a.AddSpell(PS_HealthHigh, false)
    Utility.Wait(2.0)
    String r = "probe (health >= 90%) at full: " + a.HasMagicEffect(PE_HealthHigh)
    Int i = 0
    While i < 3
        a.DamageActorValue("Health", a.GetActorValue("Health") * 0.5)
        Float t0 = Now()
        While a.HasMagicEffect(PE_HealthHigh) && Now() - t0 < 10.0
            Utility.Wait(0.05)
        EndWhile
        r += "; health halved: gone after " + Num(Now() - t0) + " s"
        a.RestoreActorValue("Health", 1000.0)
        t0 = Now()
        While !a.HasMagicEffect(PE_HealthHigh) && Now() - t0 < 10.0
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

String Function TestResistanceCap()
    String r = ""
    Float[] res = new Float[5]
    res[0] = 0.0
    res[1] = 85.0
    res[2] = 100.0
    res[3] = 150.0
    res[4] = -50.0
    Int i = 0
    While i < 5
        Actor t = Spawn(NakedBase, 0.0, 400.0)
        t.SetDontMove(true)
        t.ForceActorValue("HealRate", 0.0)
        t.ForceActorValue("FireResist", res[i])
        Utility.Wait(0.5)
        Float h = t.GetActorValue("Health")
        WhFireSpell.Cast(Player, t)
        Utility.Wait(1.5)
        r += "NPC fire resistance " + res[i] + ": a 10 point fire spell took " + Num(h - t.GetActorValue("Health")) + "\n"
        t.StopCombat()
        Gone(t)
        i += 1
    EndWhile
    return r
EndFunction

; Calm then Fury on one target: the aggression held each time; Fear on a
; level 8 target with magic resistance (does resistance lower the cap?).
String Function TestHolds()
    Actor a = Spawn(LowBase, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(0.5)
    String r = "level 1: aggression " + a.GetActorValue("Aggression")
    CalmSpell.Cast(Player, a)
    Utility.Wait(1.0)
    r += "; Calm: " + a.GetActorValue("Aggression")
    FurySpell.Cast(Player, a)
    Utility.Wait(1.0)
    r += "; then Fury: " + a.GetActorValue("Aggression") + ", confidence " + a.GetActorValue("Confidence")
    a.DispelSpell(FurySpell)
    Utility.Wait(1.0)
    r += "; Fury dispelled: " + a.GetActorValue("Aggression")
    a.DispelSpell(CalmSpell)
    Utility.Wait(1.0)
    r += "; Calm dispelled: " + a.GetActorValue("Aggression") + "\n"
    a.StopCombat()
    Gone(a)
    Float[] res = new Float[3]
    res[0] = 0.0
    res[1] = 50.0
    res[2] = 90.0
    Int i = 0
    While i < 3
        Actor b = Spawn(Level8Base, 0.0, 300.0)
        b.SetDontMove(true)
        b.ForceActorValue("MagicResist", res[i])
        Utility.Wait(0.5)
        Float conf = b.GetActorValue("Confidence")
        FearSpell.Cast(Player, b)
        Utility.Wait(1.0)
        r += "Fear on a level 8 NPC, magic resistance " + res[i] + ": confidence " + conf + " -> " + b.GetActorValue("Confidence") + "\n"
        b.StopCombat()
        Gone(b)
        i += 1
    EndWhile
    return r
EndFunction

String Function TestCloak()
    Actor[] near = new Actor[4]
    Float[] dist = new Float[4]
    dist[0] = 100.0
    dist[1] = 200.0
    dist[2] = 300.0
    dist[3] = 500.0
    Player.MoveTo(CellCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(1.0)
    Int i = 0
    While i < 4
        near[i] = Spawn(NakedBase, dist[i], 0.0)
        near[i].MoveTo(Player, dist[i], 0.0, 0.0, false)
        near[i].SetDontMove(true)
        near[i].ForceActorValue("HealRate", 0.0)
        i += 1
    EndWhile
    Utility.Wait(1.0)
    FlameCloakSpell.Cast(Player, Player)
    String r = "Flame Cloak on the player, NPCs held at 100/200/300/500 units, health lost by second:"
    Float[] h0 = new Float[4]
    i = 0
    While i < 4
        h0[i] = near[i].GetActorValue("Health")
        i += 1
    EndWhile
    Int s = 0
    While s < 6
        Utility.Wait(1.0)
        r += "\n  " + (s + 1) + " s:"
        i = 0
        While i < 4
            r += " " + Num(h0[i] - near[i].GetActorValue("Health"))
            i += 1
        EndWhile
        s += 1
    EndWhile
    r += "\n  their combat states:"
    i = 0
    While i < 4
        r += " " + near[i].GetCombatState()
        near[i].StopCombat()
        Gone(near[i])
        i += 1
    EndWhile
    Player.DispelSpell(FlameCloakSpell)
    GoCell()
    return r + "\n"
EndFunction

String Function TestEthereal()
    Actor a = Spawn(LoggedBase, 0.0, 300.0)
    a.SetDontMove(true)
    a.ForceActorValue("HealRate", 0.0)
    Utility.Wait(1.0)
    EtherealSpell.Cast(a, a)
    Utility.Wait(1.0)
    (a as VrmAt21ActorLog).Take()
    String r = "Become Ethereal cast on an NPC: has the effect " + a.HasMagicEffect(EtherealEffect)
    Float h = a.GetActorValue("Health")
    WhFireSpell.Cast(Player, a)
    Utility.Wait(1.5)
    r += "; a 10 point fire spell took " + Num(h - a.GetActorValue("Health")) + ", events " + (a as VrmAt21ActorLog).Take()
    h = a.GetActorValue("Health")
    a.DamageActorValue("Health", 10.0)
    r += "; DamageActorValue(10) took " + Num(h - a.GetActorValue("Health"))
    Float m = a.GetActorValue("Magicka")
    a.DamageActorValue("Magicka", 10.0)
    r += ", magicka 10 took " + Num(m - a.GetActorValue("Magicka")) + "\n"
    a.StopCombat()
    Gone(a)
    return r
EndFunction

; How far a dropped bowl falls in 0.4 real seconds.
Float Function Fall()
    ObjectReference b = CellCenter.PlaceAtMe(Bowl, 1, false, false)
    b.MoveTo(CellCenter, 200.0, 200.0, 500.0, false)
    Utility.Wait(0.05)
    b.SetMotionType(1, true)
    Float z = b.GetPositionZ()
    Float t0 = Now()
    Utility.Wait(0.4)
    Float fell = z - b.GetPositionZ()
    Float dt = Now() - t0
    Gone(b)
    return fell / dt
EndFunction

String Function TestSlowTime()
    String r = "a bowl's fall over 0.4 s, units per real second: before " + Num(Fall())
    Float t0 = Now()
    Float g0 = Utility.GetCurrentGameTime()
    SlowTimeSpell.Cast(Player, Player)
    Utility.Wait(0.2)
    r += "; under Slow Time 1 " + Num(Fall())
    Float w0 = Now()
    Utility.Wait(1.0)
    r += "; Wait(1.0) took " + Num(Now() - w0) + " real s"
    While Player.HasMagicEffect(SlowTimeEffect) && Now() - t0 < 60.0
        Utility.Wait(0.1)
    EndWhile
    r += "; the effect lasted " + Num(Now() - t0) + " real s, " + Num((Utility.GetCurrentGameTime() - g0) * 86400.0) + " game s"
    r += "; after: " + Num(Fall()) + "\n"
    return r
EndFunction

String Function TestSoulTrap()
    Int[] before = new Int[6]
    Player.AddItem(SoulGemPetty, 1, true)
    Player.AddItem(SoulGemGrand, 1, true)
    Player.AddItem(SoulGemBlack, 1, true)
    Utility.Wait(0.5)
    String r = "gems held: petty, grand, black (empty). "
    Actor w = Spawn(WolfBase, 0.0, 300.0)
    w.SetDontMove(true)
    Utility.Wait(1.0)
    w.Kill(Player)
    Bool trapped = Player.TrapSoul(w)
    Utility.Wait(1.0)
    r += "a wolf (petty soul) killed, TrapSoul " + trapped + ": empty petty " + Player.GetItemCount(SoulGemPetty) + ", filled petty " + Player.GetItemCount(SoulGemPettyFilled)
    r += ", empty grand " + Player.GetItemCount(SoulGemGrand) + ", empty black " + Player.GetItemCount(SoulGemBlack) + "\n"
    Actor n = Spawn(NakedBase, 300.0, 300.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    n.Kill(Player)
    trapped = Player.TrapSoul(n)
    Utility.Wait(1.0)
    r += "an NPC (black soul) killed, TrapSoul " + trapped + ": empty black " + Player.GetItemCount(SoulGemBlack) + ", filled black " + Player.GetItemCount(SoulGemBlackFilled) + ", empty grand " + Player.GetItemCount(SoulGemGrand) + "\n"
    Actor n2 = Spawn(NakedBase, -300.0, 300.0)
    n2.SetDontMove(true)
    Utility.Wait(1.0)
    n2.Kill(Player)
    trapped = Player.TrapSoul(n2)
    r += "another NPC, no black gem left: TrapSoul " + trapped + ", empty grand " + Player.GetItemCount(SoulGemGrand) + ", filled grand " + Player.GetItemCount(SoulGemGrandFilled) + "\n"
    Actor w2 = Spawn(WolfBase, 0.0, -300.0)
    w2.SetDontMove(true)
    Utility.Wait(1.0)
    w2.Kill(Player)
    trapped = Player.TrapSoul(w2)
    r += "another wolf, only the grand gem empty: TrapSoul " + trapped + ", empty grand " + Player.GetItemCount(SoulGemGrand) + ", filled grand " + Player.GetItemCount(SoulGemGrandFilled) + "\n"
    Player.RemoveItem(SoulGemPetty, Player.GetItemCount(SoulGemPetty), true, None)
    Player.RemoveItem(SoulGemPettyFilled, 1, true, None)
    Player.RemoveItem(SoulGemGrand, 1, true, None)
    Player.RemoveItem(SoulGemGrandFilled, 1, true, None)
    Player.RemoveItem(SoulGemBlack, 1, true, None)
    Player.RemoveItem(SoulGemBlackFilled, 1, true, None)
    Gone(w)
    Gone(n)
    Gone(n2)
    Gone(w2)
    return r
EndFunction

String Function TestWornEnchantment()
    Actor a = Spawn(NakedBase, 0.0, 300.0)
    a.SetDontMove(true)
    Utility.Wait(0.5)
    Float h = a.GetActorValueMax("Health")
    a.AddItem(HealthRing, 1, true)
    a.EquipItem(HealthRing, false, true)
    Utility.Wait(1.5)
    String r = "a ring of Fortify Health (" + Id(HealthRing) + ") worn: equipped " + a.IsEquipped(HealthRing) + ", max health " + Num(h) + " -> " + Num(a.GetActorValueMax("Health"))
    a.UnequipItem(HealthRing, false, true)
    Utility.Wait(1.5)
    r += ", taken off " + Num(a.GetActorValueMax("Health")) + "\n"
    Gone(a)
    return r
EndFunction

String Function TestHostileSpell()
    Actor a = Spawn(LoggedBase, 0.0, 300.0)
    a.SetDontMove(true)
    a.SetCrimeFaction(CrimeFaction)
    Utility.Wait(1.0)
    (a as VrmAt21ActorLog).Take()
    CrimeFaction.SetCrimeGold(0)
    WhFireSpell.Cast(Player, a)
    Utility.Wait(2.0)
    String r = "a fire spell cast by script from the player at a lawful NPC: events " + (a as VrmAt21ActorLog).Take() + ", combat state " + a.GetCombatState() + ", crime gold " + CrimeFaction.GetCrimeGold() + "\n"
    a.StopCombat()
    CrimeFaction.SetCrimeGold(0)
    Gone(a)
    return r
EndFunction
