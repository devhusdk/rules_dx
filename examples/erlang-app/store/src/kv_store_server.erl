-module(kv_store_server).
-behaviour(gen_server).

-export([start_link/0]).
-export([init/1, handle_call/3, handle_cast/2, handle_info/2]).

start_link() ->
    gen_server:start_link({local, ?MODULE}, ?MODULE, [], []).

init([]) ->
    {ok, Limit} = application:get_env(kv_store, max_keys),
    {ok, #{limit => Limit, data => #{}, watchers => []}}.

handle_call({put, Key, Value}, _From, State = #{limit := Limit, data := Data}) ->
    case maps:is_key(Key, Data) orelse map_size(Data) < Limit of
        true ->
            notify(State, {put, Key, Value}),
            {reply, ok, State#{data := Data#{Key => Value}}};
        false ->
            {reply, {error, full}, State}
    end;
handle_call({get, Key}, _From, State = #{data := Data}) ->
    case maps:find(Key, Data) of
        {ok, Value} -> {reply, {ok, Value}, State};
        error -> {reply, {error, not_found}, State}
    end;
handle_call({del, Key}, _From, State = #{data := Data}) ->
    notify(State, {del, Key}),
    {reply, ok, State#{data := maps:remove(Key, Data)}};
handle_call(snapshot, _From, State = #{data := Data}) ->
    {reply, Data, State};
handle_call({watch, Pid}, _From, State = #{watchers := Watchers}) ->
    {reply, ok, State#{watchers := [Pid | Watchers]}}.

handle_cast(_Message, State) ->
    {noreply, State}.

handle_info(_Message, State) ->
    {noreply, State}.

notify(#{watchers := Watchers}, Event) ->
    [Pid ! {kv_event, Event} || Pid <- Watchers],
    ok.
