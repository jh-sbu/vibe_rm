; More fights in the warehouse: crime's attack on sight, factions made
; allies mid-fight, invisibility, school cost reductions on NPCs, enchanted
; weapons' charge, combat state after StopCombat and on switching targets.

; crime.md: a guard of a faction the player owes 1000 to.
String Function TestAttackOnSight()
    Actor g = SpawnWh(CrimeGuardBase, 500.0, 0.0)
    Player.MoveTo(WhCenter, 0.0, 0.0, 0.0, false)
    Utility.Wait(2.0)
    String r = "a guard of the crime faction near the player, bounty 0: state " + g.GetCombatState()
    CrimeFaction.SetCrimeGold(999)
    Utility.Wait(4.0)
    r += "; 999: " + g.GetCombatState()
    CrimeFaction.SetCrimeGold(1000)
    Utility.Wait(4.0)
    r += "; 1000: " + g.GetCombatState() + " (target player " + (g.GetCombatTarget() == Player) + ")"
    CrimeFaction.SetCrimeGoldViolent(1000)
    CrimeFaction.SetCrimeGold(0)
    Utility.Wait(4.0)
    r += "; 1000 violent only: " + g.GetCombatState() + "\n"
    CrimeFaction.SetCrimeGoldViolent(0)
    g.StopCombat()
    Player.StopCombatAlarm()
    Gone(g)
    Player.RestoreActorValue("Health", 2000.0)
    return r
EndFunction

; factions.md: a fight between enemies whose factions are made allies.
String Function TestFightAfterAlly()
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor b = SpawnWh(GreenFighterBase, 300.0, 0.0)
    a.ForceActorValue("HealRate", 0.0)
    b.ForceActorValue("HealRate", 0.0)
    Utility.Wait(1.0)
    a.StartCombat(b)
    b.StartCombat(a)
    Utility.Wait(3.0)
    String r = "Red and Green fighting: states " + a.GetCombatState() + "/" + b.GetCombatState()
    RedFaction.SetAlly(GreenFaction, true, true)
    Int s = 0
    While s < 5
        Utility.Wait(1.0)
        r += "; allied +" + (s + 1) + " s: " + a.GetCombatState() + "/" + b.GetCombatState()
        s += 1
    EndWhile
    RedFaction.SetEnemy(GreenFaction, false, false)
    r += "\n"
    a.StopCombat()
    b.StopCombat()
    Gone(a)
    Gone(b)
    return r
EndFunction

; invisibility.md: an invisible NPC that attacks.
String Function TestInvisibleAttacker()
    Actor a = SpawnWh(AverageBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 150.0, 0.0)
    t.SetDontMove(true)
    Utility.Wait(1.0)
    InvisibilitySpell.Cast(a, a)
    Utility.Wait(1.0)
    String r = "an NPC under Invisibility (value " + a.GetActorValue("Invisibility") + ")"
    a.StartCombat(t)
    Float t0 = Now()
    While a.GetActorValue("Invisibility") > 0.0 && Now() - t0 < 15.0
        Utility.Wait(0.1)
    EndWhile
    r += " set on a target 150 off: invisible for " + Num(Now() - t0) + " s of the fight (value now " + a.GetActorValue("Invisibility") + ")\n"
    a.StopCombat()
    Gone(a)
    Gone(t)
    return r
EndFunction

; magic-menu.md: Destruction cost reduction on an NPC without the player's
; perk; casters' magicka per cast.
String Function MagickaPerCast(Float mod)
    Actor c = SpawnWh(MageBase, 0.0, 0.0)
    Actor t = SpawnWh(InvulnerableBase, 600.0, 0.0)
    t.SetDontMove(true)
    c.ForceActorValue("MagickaRate", 0.0)
    c.ForceActorValue("DestructionMod", mod)
    c.ForceActorValue("Magicka", 500.0)
    Utility.Wait(1.0)
    (c as VrmAt21ActorLog).Take()
    Float m = c.GetActorValue("Magicka")
    c.StartCombat(t)
    Utility.Wait(10.0)
    String log = (c as VrmAt21ActorLog).Take()
    String r = "DestructionMod " + mod + ": magicka spent " + Num(m - c.GetActorValue("Magicka")) + " (" + log + ")"
    c.StopCombat()
    Gone(c)
    Gone(t)
    return r
EndFunction

String Function TestNpcCostReduction()
    return MagickaPerCast(0.0) + "\n" + MagickaPerCast(50.0) + "\n" + MagickaPerCast(-50.0) + "\n"
EndFunction

; staffs.md: an NPC's enchanted sword's charge per blow.
String Function TestEnchantedBlows()
    Actor a = SpawnWh(AverageBase, 0.0, 0.0)
    Actor t = SpawnWh(LoggedBase, 120.0, 0.0)
    t.SetDontMove(true)
    t.ForceActorValue("Health", 5000.0)
    Utility.Wait(1.0)
    a.AddItem(FireSword, 1, true)
    a.EquipItem(FireSword, false, true)
    Utility.Wait(1.0)
    (t as VrmAt21ActorLog).Take()
    String r = "an NPC with an iron sword of burning: charge " + Num(a.GetActorValue("RightItemCharge"))
    a.StartCombat(t)
    Utility.Wait(10.0)
    r += "; after 10 s fighting: charge " + Num(a.GetActorValue("RightItemCharge")) + ", the target's events " + (t as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    Gone(a)
    Gone(t)
    return r
EndFunction

; combat-state.md: StopCombat on a hostile actor; switching targets.
String Function TestStopCombat()
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor b = SpawnWh(GreenFighterBase, 300.0, 0.0)
    Actor c = SpawnWh(GreenFighterBase, -300.0, 0.0)
    a.ForceActorValue("Health", 5000.0)
    b.ForceActorValue("Health", 5000.0)
    c.ForceActorValue("Health", 5000.0)
    Utility.Wait(1.0)
    (a as VrmAt21ActorLog).Take()
    a.StartCombat(b)
    Utility.Wait(3.0)
    a.StopCombat()
    Float t0 = Now()
    String r = "Red fighting Green, StopCombat: state at once " + a.GetCombatState()
    While a.GetCombatState() == 0 && Now() - t0 < 10.0
        Utility.Wait(0.05)
    EndWhile
    r += ", fighting again after " + Num(Now() - t0) + " s"
    a.StartCombat(c)
    Utility.Wait(2.0)
    r += "; StartCombat on another (target now the other " + (a.GetCombatTarget() == c) + "): events " + (a as VrmAt21ActorLog).Take() + "\n"
    a.StopCombat()
    b.StopCombat()
    c.StopCombat()
    Gone(a)
    Gone(b)
    Gone(c)
    return r
EndFunction

; combat-state.md: IsInCombat's integer parameter, during a fight.
String Function TestInCombatParam()
    Actor a = SpawnWh(RedFighterBase, 0.0, 0.0)
    Actor b = SpawnWh(GreenFighterBase, 300.0, 0.0)
    a.ForceActorValue("Health", 5000.0)
    b.ForceActorValue("Health", 5000.0)
    Utility.Wait(1.0)
    a.StartCombat(b)
    Utility.Wait(2.0)
    String r = "fighting (state " + a.GetCombatState() + "): IsInCombat(0) " + Probe(a, PS_InCombat0, PE_InCombat0) + ", (1) " + Probe(a, PS_InCombat1, PE_InCombat1) + ", (2) " + Probe(a, PS_InCombat2, PE_InCombat2) + "\n"
    a.StopCombat()
    b.StopCombat()
    Utility.Wait(1.0)
    r += "after StopCombat (state " + a.GetCombatState() + "): (0) " + Probe(a, PS_InCombat0, PE_InCombat0) + ", (1) " + Probe(a, PS_InCombat1, PE_InCombat1) + "\n"
    Gone(a)
    Gone(b)
    return r
EndFunction
