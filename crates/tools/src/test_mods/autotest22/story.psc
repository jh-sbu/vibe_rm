; Story Manager pickups and quest aliases' created references, in the test
; cell. Each story quest stops itself after reporting; autotest21 read the
; next event 1.5 s later, maybe before the quest had stopped. Now 4 s.

String StoryLog = ""

Function AddStory(String s)
    StoryLog = StoryLog + s + "@" + Math.Floor(Utility.GetCurrentRealTime() * 100.0) + "; "
EndFunction

String Function TakeStory()
    Utility.Wait(4.0)
    String s = StoryLog
    StoryLog = ""
    If s == ""
        return "(none)"
    EndIf
    return s
EndFunction

String Function TestStoryPickups()
    String r = "(player " + Id(Player) + "); left from earlier tests: " + TakeStory() + "\n"
    ObjectReference a = Put(IngotIron, 0.0, 150.0, 20.0)
    Utility.Wait(0.5)
    a.Activate(Player, false)
    r += "an unowned ingot picked up: " + TakeStory() + "\n"
    ObjectReference b = Put(IngotIron, 0.0, 150.0, 20.0)
    b.SetActorOwner(NakedBase)
    Utility.Wait(0.5)
    b.Activate(Player, false)
    r += "an ingot owned by an NPC base picked up: " + TakeStory() + "\n"
    ObjectReference c = Put(IngotIron, 0.0, 150.0, 20.0)
    c.SetFactionOwner(CrimeFaction)
    Utility.Wait(0.5)
    c.Activate(Player, false)
    r += "an ingot owned by a faction picked up: " + TakeStory() + "\n"
    ObjectReference chest = Put(EmptyChest, 200.0, 0.0, 0.0)
    chest.AddItem(IngotIron, 1, true)
    chest.RemoveItem(IngotIron, 1, false, Player)
    r += "taken from a chest by RemoveItem: " + TakeStory() + "\n"
    Player.RemoveItem(IngotIron, 1, false, chest)
    r += "put in the chest by RemoveItem: " + TakeStory() + "\n"
    Player.RemoveItem(IngotIron, Player.GetItemCount(IngotIron), true, None)
    CrimeFaction.SetCrimeGold(0)
    Gone(chest)
    return r
EndFunction

; alias-fills: what happens to an alias's created references (the anchor
; is now the player, and the aliases optional, as in autotest4's).
String Function RefState(ObjectReference o)
    If o == None
        return "none"
    EndIf
    return Id(o) + " disabled " + o.IsDisabled() + " deleted " + o.IsDeleted() + " 3D " + o.Is3DLoaded() + " held by the player " + (Player.GetItemCount(o.GetBaseObject()) > 0)
EndFunction

String Function TestCreatedRefs()
    Bool started = CreateQuest.Start()
    Utility.Wait(1.0)
    ReferenceAlias made = CreateQuest.GetAlias(1) as ReferenceAlias
    ReferenceAlias item = CreateQuest.GetAlias(2) as ReferenceAlias
    ObjectReference a1 = made.GetReference()
    ObjectReference i1 = item.GetReference()
    String r = "Start " + started + ", running " + CreateQuest.IsRunning() + "; created: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    made.Clear()
    item.Clear()
    Utility.Wait(1.0)
    r += "aliases cleared: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    CreateQuest.Stop()
    Utility.Wait(2.0)
    r += "quest stopped: actor " + RefState(a1) + "; item " + RefState(i1) + "\n"
    CreateQuest.Start()
    Utility.Wait(1.0)
    ObjectReference a2 = made.GetReference()
    ObjectReference i2 = item.GetReference()
    r += "started again: actor " + RefState(a2) + "; item " + RefState(i2) + "\n"
    CreateQuest.CompleteQuest()
    Utility.Wait(1.0)
    r += "CompleteQuest: actor " + RefState(a2) + "; item " + RefState(i2) + ", running " + CreateQuest.IsRunning() + "\n"
    CreateQuest.Stop()
    Utility.Wait(2.0)
    r += "then stopped: actor " + RefState(a2) + "; item " + RefState(i2) + "\n"
    Gone(a1)
    Gone(a2)
    Gone(i1)
    Gone(i2)
    Player.RemoveItem(IronSword, Player.GetItemCount(IronSword), true, None)
    return r
EndFunction

; alias-fills: a leveled bandit created at create levels easy and very hard.
String Function TestCreateLevel()
    String r = "player level " + Player.GetLevel()
    Int i = 0
    While i < 2
        Quest q = LevelQuests.GetAt(i) as Quest
        q.Start()
        Utility.Wait(1.0)
        Actor made = (q.GetAlias(1) as ReferenceAlias).GetActorReference()
        If made == None
            r += "; level " + (i * 3) + ": none"
        Else
            r += "; level " + (i * 3) + ": level " + made.GetLevel() + " base " + Id(made.GetLeveledActorBase()) + " race " + Id(made.GetRace())
        EndIf
        q.Stop()
        Gone(made)
        i += 1
    EndWhile
    return r + "\n"
EndFunction
