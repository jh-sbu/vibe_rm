; autotest23's faults in WarehouseFurniture. The player gets 1000 extra
; health here and loses it on leaving.

String Function TestEnterWarehouse()
    GoWarehouse()
    Player.ModActorValue("Health", 1000.0)
    Player.RestoreActorValue("Health", 1000.0)
    return "in the warehouse: interior " + Player.IsInInterior() + ", WhCenter distance " + Num(Player.GetDistance(WhCenter)) + "\n"
EndFunction

; magic: Fear, Calm and Fury cast from 150 units (autotest23's Calm and Fury
; from about 550 didn't land), each read for 8 s: distance, combat state
; and the actor value each holds, then IsFleeing; Fear on levels 1 and 8.
String Function Near(Actor a)
    Player.MoveTo(a, 0.0, -150.0, 0.0, false)
    Utility.Wait(0.5)
    return " (player " + Num(Player.GetDistance(a)) + " off)"
EndFunction

String Function FearCase(ActorBase b, String label)
    Actor f = SpawnWh(b, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    f.AddItem(IronSword, 1, true)
    f.EquipItem(IronSword, false, true)
    Utility.Wait(1.0)
    f.StartCombat(t)
    Utility.Wait(3.0)
    (f as VrmAt21ActorLog).Take()
    String r = "Fear on " + label + " (level " + f.GetLevel() + ")" + Near(f) + ": distance/state/confidence every 2 s:"
    FearSpell.Cast(Player, f)
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        r += " " + Num(f.GetDistance(t)) + "/" + f.GetCombatState() + "/" + Num(f.GetActorValue("Confidence"))
        s += 1
    EndWhile
    r += "; IsFleeing " + Probe(f, PS_Fleeing, PE_Fleeing) + ", events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    Gone(f)
    Gone(t)
    Player.MoveTo(WhCenter, -500.0, 0.0, 0.0, false)
    Player.RestoreActorValue("Health", 2000.0)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestHolds()
    String r = FearCase(LowFighterLoggedBase, "a level 1 fighter") + FearCase(Level8LoggedBase, "a level 8 NPC")
    Actor c = SpawnWh(LowFighterLoggedBase, 0.0, 0.0)
    Actor ct = SpawnWh(InvulnerableBase, 200.0, 0.0)
    ct.SetDontMove(true)
    Utility.Wait(1.0)
    c.StartCombat(ct)
    Utility.Wait(3.0)
    (c as VrmAt21ActorLog).Take()
    r += "Calm on a level 1 fighter" + Near(c) + " (combat " + c.GetCombatState() + ", aggression " + Num(c.GetActorValue("Aggression")) + "): state/aggression every 2 s:"
    CalmSpell.Cast(Player, c)
    Int s = 0
    While s < 4
        Utility.Wait(2.0)
        r += " " + c.GetCombatState() + "/" + Num(c.GetActorValue("Aggression"))
        s += 1
    EndWhile
    r += "; events " + (c as VrmAt21ActorLog).Take() + "\n"
    c.StopCombat()
    Gone(c)
    Gone(ct)
    Actor f = SpawnWh(LowLoggedBase, 0.0, 0.0)
    Actor n = SpawnWh(NakedBase, 200.0, 0.0)
    n.SetDontMove(true)
    Utility.Wait(1.0)
    (f as VrmAt21ActorLog).Take()
    r += "Fury on a level 1 unaggressive NPC" + Near(f) + ", another 200 off: state/target/aggression every 2 s:"
    FurySpell.Cast(Player, f)
    s = 0
    While s < 4
        Utility.Wait(2.0)
        String who = "none"
        If f.GetCombatTarget() == n
            who = "other"
        ElseIf f.GetCombatTarget() == Player
            who = "player"
        EndIf
        r += " " + f.GetCombatState() + "/" + who + "/" + Num(f.GetActorValue("Aggression"))
        s += 1
    EndWhile
    r += "; events " + (f as VrmAt21ActorLog).Take() + "\n"
    f.StopCombat()
    n.StopCombat()
    Player.StopCombatAlarm()
    Gone(f)
    Gone(n)
    Player.MoveTo(WhCenter, -500.0, 0.0, 0.0, false)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; npc-healing: what the Vigilant casts as it is hurt (the log now names
; each spell cast), at 90, 70, 55, 45 and 20 percent, its magicka full.
String Function TestHealer()
    Actor h = SpawnWh(VigilantLogBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 200.0, 0.0)
    t.SetDontMove(true)
    h.ForceActorValue("HealRate", 0.0)
    h.SetActorValue("Magicka", 300.0)
    h.RestoreActorValue("Magicka", 1000.0)
    Utility.Wait(2.0)
    (h as VrmAt21ActorLog).Take()
    h.StartCombat(t)
    Utility.Wait(2.0)
    String r = "a Vigilant (level " + h.GetLevel() + ", Restoration " + Num(h.GetActorValue("Restoration")) + ") fighting; spells right " + Id(h.GetEquippedSpell(1)) + ", left " + Id(h.GetEquippedSpell(0)) + "; by health fraction:"
    Float[] at = new Float[5]
    at[0] = 0.9
    at[1] = 0.7
    at[2] = 0.55
    at[3] = 0.45
    at[4] = 0.2
    Int i = 0
    While i < 5
        h.RestoreActorValue("Health", 10000.0)
        h.RestoreActorValue("Magicka", 1000.0)
        h.DamageActorValue("Health", h.GetActorValueMax("Health") * (1.0 - at[i]))
        Float before = h.GetActorValuePercentage("Health")
        Float m = h.GetActorValue("Magicka")
        Utility.Wait(8.0)
        r += "\n  at " + Num(before) + " -> " + Num(h.GetActorValuePercentage("Health")) + ", magicka " + Num(m) + " -> " + Num(h.GetActorValue("Magicka")) + ", spells " + Id(h.GetEquippedSpell(1)) + " / " + Id(h.GetEquippedSpell(0)) + " (" + (h as VrmAt21ActorLog).Take() + ")"
        i += 1
    EndWhile
    h.StopCombat()
    Gone(h)
    Gone(t)
    return r + "\n"
EndFunction

; What an NPC's view of the player is.
String Function Views(Actor a)
    return "hostile " + a.IsHostileToActor(Player) + ", detects " + Player.IsDetectedBy(a) + ", state " + a.GetCombatState() + ", targets the player " + (a.GetCombatTarget() == Player)
EndFunction

; crime: Attack on Sight with a guard that IsGuard (autotest23's wasn't).
String Function TestAttackOnSightGuard()
    Actor g = SpawnWh(AosIsGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(3.0)
    String r = "a guard (IsGuard " + g.IsGuard() + ", crime faction " + Id(g.GetCrimeFaction()) + ") of a faction with Arrest and Attack on Sight, 500 off: bounty 0, " + Views(g) + "\n"
    AosFaction.SetCrimeGold(999)
    Utility.Wait(6.0)
    r += "bounty 999 +6 s: " + Views(g) + ", distance " + Num(g.GetDistance(Player)) + "\n"
    AosFaction.SetCrimeGold(1000)
    Utility.Wait(6.0)
    r += "bounty 1000 +6 s: " + Views(g) + ", distance " + Num(g.GetDistance(Player)) + "\n"
    AosFaction.SetCrimeGold(0)
    g.StopCombat()
    Player.StopCombatAlarm()
    Gone(g)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; combat-assistance: the case autotest23 lost (its attacker never fought):
; a helper of friends and allies at rank 4, with rank 3 as the control.
String Function HelpCase(ActorBase hb, Int rank, String label)
    Actor v = SpawnWh(NakedBase, 0.0, 0.0)
    Actor a = SpawnWh(RedFighterBase, 250.0, 0.0)
    Actor h = SpawnWh(hb, -400.0, 0.0)
    v.SetDontMove(true)
    v.ForceActorValue("Health", 5000.0)
    a.ForceActorValue("Health", 5000.0)
    h.SetRelationshipRank(v, rank)
    v.SetRelationshipRank(h, rank)
    Utility.Wait(1.5)
    a.StartCombat(v)
    Utility.Wait(8.0)
    String r = "  " + label + ", rank " + h.GetRelationshipRank(v) + ": attacker state " + a.GetCombatState() + ", victim state " + v.GetCombatState() + ", helper state " + h.GetCombatState() + ", helper targets the attacker " + (h.GetCombatTarget() == a) + "\n"
    a.StopCombat()
    v.StopCombat()
    h.StopCombat()
    Gone(a)
    Gone(v)
    Gone(h)
    Utility.Wait(1.0)
    return r
EndFunction

String Function TestRelationshipHelp()
    String r = "a Red fighter attacks a held victim; a helper 650 off:\n"
    r += HelpCase(HelperBase, 4, "helps friends and allies")
    r += HelpCase(HelperBase, 3, "helps friends and allies")
    r += HelpCase(HelperBase, 4, "helps friends and allies, again")
    return r
EndFunction

String Function TestLeaveWarehouse()
    Player.ModActorValue("Health", -1000.0)
    GoCell()
    return "back in the test cell\n"
EndFunction
