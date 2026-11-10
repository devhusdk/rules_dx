<script lang="ts">
let   count:number=0
  function  inc(){count+=1}
</script>

<button   on:click={inc}>
clicked {count}
</button>

<style>
button{color:red;
    margin:0}
</style>
